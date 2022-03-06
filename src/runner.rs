#![allow(clippy::needless_collect, clippy::type_complexity)]

use crate::{
    env::RunnerEnv,
    metrics::{
        disk_io::DiskIoMeasurement, flamegraph::FlamegraphMeasurement, Measurement,
        PerThreadMeasurement,
    },
    result::{BenchValue, ShumaiResult, ThreadResult},
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

struct Runner<'a, B: ShumaiBench> {
    f: &'a mut B,
    threads: Vec<usize>,
    config: &'a B::Config,
    repeat: usize,
    running_time: Duration,
    measure: Vec<Box<dyn Measurement>>,
    per_thread_measure: Vec<Box<dyn PerThreadMeasurement>>,
}

impl<'a, B: ShumaiBench> Runner<'a, B> {
    fn new(f: &'a mut B, config: &'a B::Config, repeat: usize) -> Self {
        let (repeat, running_time) = match is_profile_by_time() {
            Some(t) => (1, Duration::from_secs(t as u64)),
            None => (repeat, Duration::from_secs(config.bench_sec() as u64)),
        };

        let threads = match std::env::var("SHUMAI_THREAD") {
            Ok(s) => {
                let t = s.parse::<usize>().expect("SHUMAI_THREAD must be a number");
                eprintln!(
                    "Using environment variable SHUMAI_THREAD to set thread count to {}",
                    t
                );
                config
                    .thread()
                    .iter()
                    .filter(|ct| **ct == t)
                    .copied()
                    .collect::<Vec<_>>()
            }
            Err(_) => config.thread().to_vec(),
        };
        let measurements: Vec<Box<dyn Measurement>> = vec![
            Box::new(DiskIoMeasurement::new()),
            Box::new(FlamegraphMeasurement::new()),
        ];

        Self {
            f,
            config,
            repeat,
            running_time,
            threads,
            measure: measurements,
            per_thread_measure: vec![],
        }
    }

    #[must_use]
    fn load(&mut self) -> Option<serde_json::Value> {
        print_loading();
        self.f.load()
    }

    fn threads(&self) -> Vec<usize> {
        self.threads.clone()
    }

    fn bench_thread(&mut self, thread_cnt: usize) -> ThreadResult<B::Result> {
        let mut sample_results = Vec::new();

        print_running(
            self.running_time.as_secs() as usize,
            self.config.name(),
            thread_cnt,
        );

        for i in 0..self.repeat {
            let sample_result = self.bench_one_iter(thread_cnt);

            self.f.on_iteration_finished(i);

            println!(
                "Iteration {} finished------------------\n{}\n",
                i, sample_result.result
            );

            sample_results.push(sample_result);
        }

        self.f.on_thread_finished(thread_cnt);

        ThreadResult {
            thread_cnt,
            iterations: sample_results,
            #[cfg(feature = "pcm")]
            pcm: sample_results.iter().last().unwrap().pcm.clone(), // only from the last sample, or it will be too verbose
            #[cfg(feature = "perf")]
            perf: sample_results.iter().last().unwrap().perf.clone(), // same as above
        }
    }

    fn bench_one_iter(&mut self, thread_cnt: usize) -> BenchValue<B::Result> {
        let ready_thread = AtomicU64::new(0);
        let is_running = AtomicBool::new(false);

        crossbeam_utils::thread::scope(|scope| {
            #[cfg(feature = "pcm")]
            let mut pcm_stats = Vec::new();

            let _thread_guard = ThreadPoison;
            let handlers: Vec<_> = (0..thread_cnt)
                .map(|tid| {
                    let context = Context {
                        thread_id: tid,
                        thread_cnt,
                        config: self.config,
                        ready_thread: &ready_thread,
                        running: &is_running,
                    };

                    scope.spawn(|_| {
                        let _thread_guard = ThreadPoison;

                        for m in self.per_thread_measure.iter() {
                            m.start();
                        }

                        #[cfg(feature = "perf")]
                        let result = { crate::metrics::perf::perf_of_func(|| f.run(context)) };

                        #[cfg(not(feature = "perf"))]
                        let result = { (self.f.run(context),) };

                        for m in self.per_thread_measure.iter() {
                            m.stop();
                        }

                        result
                    })
                })
                .collect();

            let backoff = crossbeam_utils::Backoff::new();
            while ready_thread.load(Ordering::SeqCst) != thread_cnt as u64 {
                backoff.snooze();
            }

            for m in self.measure.iter_mut() {
                m.start();
            }

            // now all threads start running!
            is_running.store(true, Ordering::SeqCst);

            let start_time = Instant::now();

            #[cfg(feature = "pcm")]
            let mut time_cnt = 0;

            while (Instant::now() - start_time) < self.running_time {
                std::thread::sleep(Duration::from_millis(50));

                #[cfg(feature = "pcm")]
                {
                    // roughly every second
                    time_cnt += 1;
                    if time_cnt % 20 == 0 {
                        let stats = crate::metrics::pcm::PcmStats::from_request();
                        pcm_stats.push(stats);
                    }
                }
            }

            // stop the world!
            is_running.store(false, Ordering::SeqCst);

            for i in self.measure.iter_mut() {
                i.stop();
            }

            let all_results = handlers
                .into_iter()
                .map(|f| f.join().unwrap())
                .collect::<Vec<_>>();

            // aggregate throughput numbers
            let thrput = all_results.iter().fold(B::Result::default(), |v, h| {
                v + h.0.clone().normalize_time(&self.running_time)
            });

            // aggregate perf numbers
            #[cfg(feature = "perf")]
            let perf_counter = all_results
                .into_iter()
                .fold(crate::metrics::perf::PerfCounter::new(), |a, mut b| {
                    a + b.1.get_stats().unwrap()
                });

            let measurements = self.measure.iter_mut().map(|m| m.result()).collect();

            BenchValue {
                result: thrput,
                measurements,
                #[cfg(feature = "perf")]
                perf: perf_counter,
                #[cfg(feature = "pcm")]
                pcm: pcm_stats,
            }
        })
        .unwrap()
    }
}

#[must_use = "bench function returns the bench results"]
pub fn run<B: ShumaiBench>(
    bench: &mut B,
    config: &B::Config,
    repeat: usize,
) -> ShumaiResult<B::Config, B::Result> {
    let mut runner = Runner::new(bench, config, repeat);
    let load_results = runner.load();
    let mut results: ShumaiResult<B::Config, B::Result> =
        ShumaiResult::new(config.clone(), load_results, RunnerEnv::new());

    let threads = runner.threads();
    for t in threads {
        let thread_results = runner.bench_thread(t);
        results.add(thread_results);
    }

    let cleanup_result = bench.cleanup();
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
