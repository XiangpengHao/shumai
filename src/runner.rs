#![allow(clippy::needless_collect)]

use crate::env::RunnerEnv;
use crate::result::{PerThreadResult, ShumaiResult};
use crate::{counters::pcm::PcmStats, BenchConfig, BenchContext, ShumaiBench};
use colored::Colorize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

fn bench_thread<B: ShumaiBench>(
    thread_cnt: usize,
    config: &B::Config,
    sample_size: usize,
    f: &B,
) -> (Vec<<B as ShumaiBench>::Result>, Vec<PcmStats>) {
    let (sample, running_time) = match is_profile_by_time() {
        Some(t) => (1, Duration::from_secs(t as u64)),
        None => (sample_size, Duration::from_secs(config.bench_sec() as u64)),
    };

    let mut bench_results = Vec::new();
    let mut pcm_stats = Vec::new();

    for i in 0..sample {
        let ready_thread = AtomicU64::new(0);
        let is_running = AtomicBool::new(false);

        crossbeam_utils::thread::scope(|scope| {
            let handlers: Vec<_> = (0..thread_cnt)
                .map(|tid| {
                    let context = BenchContext {
                        thread_id: tid,
                        thread_cnt,
                        config,
                        ready_thread: &ready_thread,
                        running: &is_running,
                    };

                    scope.spawn(|_| f.run(context))
                })
                .collect();

            let backoff = crossbeam_utils::Backoff::new();
            while ready_thread.load(Ordering::SeqCst) != thread_cnt as u64 {
                backoff.snooze();
            }

            // if flamegraph feature is enabled and we are the last sample
            let guard = if cfg!(feature = "flamegraph") && i == (sample - 1) {
                Some(pprof::ProfilerGuard::new(200).unwrap())
            } else {
                None
            };

            // now all threads start running!
            is_running.store(true, Ordering::SeqCst);

            let start_time = Instant::now();

            // TODO: we can have an async runtime here to run multiple things
            // e.g. collecting metrics
            let mut time_cnt = 0;
            while Instant::now() - start_time < running_time {
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

            let all_results: Vec<_> = handlers.into_iter().map(|f| f.join().unwrap()).collect();

            // save the flamegraph
            if let Some(guard) = guard {
                if let Ok(report) = guard.report().build() {
                    let file = std::fs::File::create(format!("{}.svg", config.name())).unwrap();
                    report.flamegraph(file).unwrap();
                }
            }

            let thrput = all_results
                .iter()
                .fold(B::Result::default(), |v, h| v + h.clone());

            println!("Iteration {} finished------------------\n{}\n", i, thrput);

            bench_results.push(thrput);
        })
        .unwrap();
    }

    (bench_results, pcm_stats)
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

#[must_use = "bench function returns the bench results"]
pub fn run<B: ShumaiBench>(
    f: &B,
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

        let (bench_results, pcm_stats) = bench_thread(*thread_cnt as usize, config, repeat, f);

        results.add_result(PerThreadResult {
            thread_cnt: *thread_cnt,
            bench_results,
            pcm: pcm_stats,
        });
    }

    f.cleanup();

    results
}
