#![allow(clippy::needless_collect, clippy::type_complexity)]

use crate::{
    env::RunnerEnv,
    metrics::{
        disk_io::{disk_io_of_func, DiskUsage},
        flamegraph::flamegraph_of_func,
        pcm::PcmStats,
        perf::{PerfCounter, PerfStatsRaw},
    },
    result::{ShumaiResult, ThreadResult},
    BenchConfig, BenchResult, Context, ShumaiBench,
};
use colored::Colorize;
use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};

struct ThreadPoison;
impl Drop for ThreadPoison {
    fn drop(&mut self) {
        if std::thread::panicking() {
            println!(
                "Benchmark thread {:?} panicked, terminating all other threads...",
                std::thread::current().id()
            );
            std::process::exit(1);
        }
    }
}

fn bench_one_sample<B: ShumaiBench>(
    thread_cnt: usize,
    config: &B::Config,
    running_time: Duration,
    f: &B,
) -> (B::Result, Option<PerfCounter>, Vec<PcmStats>, DiskUsage) {
    let ready_thread = AtomicU64::new(0);
    let is_running = AtomicBool::new(false);
    let mut pcm_stats = Vec::new();

    crossbeam::thread::scope(|scope| {
        let _thread_guard = ThreadPoison;
        let handlers: Vec<_> = (0..thread_cnt)
            .map(|tid| {
                let context = Context {
                    thread_id: tid,
                    thread_cnt,
                    config,
                    ready_thread: &ready_thread,
                    running: &is_running,
                };

                scope.spawn(|_| {
                    let _thread_guard = ThreadPoison;
                    let perf_stats = if cfg!(feature = "perf") {
                        let mut perf = PerfStatsRaw::new();
                        perf.enable().expect("unable to enable perf");
                        Some(perf)
                    } else {
                        None
                    };

                    let rv = f.run(context);

                    let perf_stats = perf_stats.map(|mut p| {
                        p.disable().expect("unable to disable perf");
                        p
                    });

                    (rv, perf_stats)
                })
            })
            .collect();

        let backoff = crossbeam::utils::Backoff::new();
        while ready_thread.load(Ordering::SeqCst) != thread_cnt as u64 {
            backoff.snooze();
        }

        let (disk_usage, all_results) = flamegraph_of_func(config.name(), || {
            disk_io_of_func(|| {
                // now all threads start running!
                is_running.store(true, Ordering::SeqCst);

                let start_time = Instant::now();

                let mut time_cnt = 0;
                while (Instant::now() - start_time) < running_time {
                    std::thread::sleep(Duration::from_millis(50));
                    time_cnt += 1;

                    if cfg!(feature = "pcm") {
                        // roughly every second
                        if time_cnt % 20 == 0 {
                            let stats = PcmStats::from_request();
                            pcm_stats.push(stats);
                        }
                    }
                }

                // stop the world!
                is_running.store(false, Ordering::SeqCst);

                handlers
                    .into_iter()
                    .map(|f| f.join().unwrap())
                    .collect::<Vec<_>>()
            })
        });

        // aggregate throughput numbers
        let thrput = all_results.iter().fold(B::Result::default(), |v, h| {
            v + h.0.clone().normalize_time(&running_time)
        });

        // aggregate perf numbers
        let perf_counter = if all_results.first().unwrap().1.is_none() {
            None
        } else {
            Some(
                all_results
                    .into_iter()
                    .fold(PerfCounter::new(), |a, mut b| {
                        return a + b.1.as_mut().unwrap().get_stats().unwrap();
                    }),
            )
        };

        (thrput, perf_counter, pcm_stats, disk_usage)
    })
    .unwrap()
}

fn bench_thread<B: ShumaiBench>(
    thread_cnt: usize,
    config: &B::Config,
    sample_size: usize,
    f: &mut B,
) -> (
    Vec<<B as ShumaiBench>::Result>,
    Vec<Option<PerfCounter>>,
    Vec<Vec<PcmStats>>,
    Vec<DiskUsage>,
) {
    let (sample, running_time) = match is_profile_by_time() {
        Some(t) => (1, Duration::from_secs(t as u64)),
        None => (sample_size, Duration::from_secs(config.bench_sec() as u64)),
    };

    let mut bench_results = Vec::new();
    let mut perf_counters = Vec::new();
    let mut pcm = Vec::new();
    let mut disk = Vec::new();

    for i in 0..sample {
        let (thrput, perf_counter, pcm_stats, disk_usage) =
            bench_one_sample(thread_cnt, config, running_time, f);

        f.on_iteration_finished(sample);

        println!("Iteration {} finished------------------\n{}\n", i, thrput);

        bench_results.push(thrput);
        perf_counters.push(perf_counter);
        pcm.push(pcm_stats);
        disk.push(disk_usage);
    }

    (bench_results, perf_counters, pcm, disk)
}

#[must_use = "bench function returns the bench results"]
pub fn run<B: ShumaiBench>(
    f: &mut B,
    config: &B::Config,
    repeat: usize,
) -> ShumaiResult<B::Config, B::Result> {
    let running_time = match is_profile_by_time() {
        Some(t) => Duration::from_secs(t as u64),
        None => Duration::from_secs(config.bench_sec() as u64),
    };

    print_loading();
    let load_result = f.load();
    let mut results: ShumaiResult<B::Config, B::Result> =
        ShumaiResult::new(config.clone(), load_result, RunnerEnv::new());

    for thread_cnt in config.thread().iter() {
        print_running(
            running_time.as_secs() as usize,
            config.name(),
            *thread_cnt as usize,
        );

        let (bench_results, perf_counter, pcm_stats, disk_usage) =
            bench_thread(*thread_cnt as usize, config, repeat, f);

        results.add_result(ThreadResult {
            thread_cnt: *thread_cnt,
            results: bench_results,
            pcm: pcm_stats.into_iter().last().unwrap(), // only from the last sample, or it will be too verbose
            perf: perf_counter.into_iter().last().unwrap(), // same as above
            disk_usage: disk_usage.into_iter().last().unwrap(), // same as above
        });

        f.on_thread_finished(*thread_cnt);
    }

    let cleanup_result = f.cleanup();
    results.cleanup_results = cleanup_result;

    results
}

fn is_profile_by_time() -> Option<usize> {
    let profile_time = std::env::var("PROFILE_TIME").ok()?;
    profile_time.parse::<usize>().ok()
}

fn print_loading() {
    println!(
        "{}\n{}",
        "============================================================".red(),
        "Loading data...".cyan()
    );
}

fn print_running(running_time: usize, name: &str, thread_cnt: usize) {
    println!(
        "{}\n{}",
        "============================================================".red(),
        format!(
            "Running benchmark for {} seconds with {} threads: {}",
            running_time, thread_cnt, name
        )
        .cyan()
    );
}
