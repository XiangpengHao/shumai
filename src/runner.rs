#![allow(clippy::needless_collect, clippy::type_complexity)]

use crate::{
    env::RunnerEnv,
    metrics::Measurement,
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
            Box::new(crate::metrics::disk_io::DiskIoMeasurement::new()),
            #[cfg(feature = "flamegraph")]
            Box::new(crate::metrics::flamegraph::FlamegraphMeasurement::new()),
            #[cfg(feature = "perf")]
            Box::new(crate::metrics::perf::PerfMeasurement::new()),
            #[cfg(feature = "pcm")]
            Box::new(crate::metrics::pcm::PcmMeasurement::new()),
        ];

        Self {
            f,
            config,
            repeat,
            running_time,
            threads,
            measure: measurements,
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
        let mut iter_results = Vec::new();

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

            iter_results.push(sample_result);
        }

        self.f.on_thread_finished(thread_cnt);

        ThreadResult {
            thread_cnt,
            iterations: iter_results,
        }
    }

    fn bench_one_iter(&mut self, thread_cnt: usize) -> BenchValue<B::Result> {
        let ready_thread = AtomicU64::new(0);
        let is_running = AtomicBool::new(false);

        std::thread::scope(|scope| {
            let _thread_guard = ThreadPoison;
            let handlers: Vec<_> = (0..thread_cnt)
                .map(|tid| {
                    let context =
                        Context::new(tid, thread_cnt, self.config, &ready_thread, &is_running);
                    scope.spawn(|| {
                        let _thread_guard = ThreadPoison;

                        self.f.run(context)
                    })
                })
                .collect();

            while ready_thread.load(Ordering::SeqCst) != thread_cnt as u64 {
                std::thread::sleep(Duration::from_millis(1));
            }

            for m in self.measure.iter_mut() {
                m.start();
            }

            // now all threads start running!
            is_running.store(true, Ordering::SeqCst);

            let start_time = Instant::now();

            while (Instant::now() - start_time) < self.running_time {
                std::thread::sleep(Duration::from_millis(50));
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
                v + h.clone().normalize_time(&self.running_time)
            });

            let measurements = self.measure.iter_mut().map(|m| m.result()).collect();

            BenchValue {
                result: thrput,
                measurements,
            }
        })
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
