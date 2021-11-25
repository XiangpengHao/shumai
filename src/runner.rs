use crate::env::RunnerEnv;
use crate::result::BenchData;
use crate::{pcm::PcmStats, BenchConfig, BenchContext, MultiThreadBench};
use colored::Colorize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct Runner {
    sample_size: usize,
}

impl Runner {
    pub fn new() -> Self {
        Self { sample_size: 5 }
    }

    pub fn sample_size(mut self, sample_size: usize) -> Self {
        self.sample_size = sample_size;
        self
    }

    fn profile_time(&self) -> Option<usize> {
        let profile_time = std::env::var("PROFILE_TIME").ok()?;
        profile_time.parse::<usize>().ok()
    }

    fn bench_one<B: MultiThreadBench + 'static>(
        &self,
        thread_cnt: usize,
        f: &B,
    ) -> (Vec<<B as MultiThreadBench>::Result>, Vec<PcmStats>) {
        let (sample, running_time) = match self.profile_time() {
            Some(t) => (1, Duration::from_secs(t as u64)),
            None => (
                self.sample_size,
                Duration::from_secs(f.get_config().bench_sec() as u64),
            ),
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
                            ready_thread: &ready_thread,
                            running: &is_running,
                        };

                        scope.spawn(|_| {
                            let result = f.run(context);
                            result
                        })
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
                        let file = std::fs::File::create(format!("{}.svg", f.get_config().name()))
                            .unwrap();
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

            f.cleanup();
        }

        (bench_results, pcm_stats)
    }

    fn extract_bench_results<B: MultiThreadBench + 'static>(
        &self,
        f: &B,
        thread_cnt: usize,
        bench_results: Vec<B::Result>,
        pcm_stats: Vec<PcmStats>,
    ) -> BenchData<B::Config, B::Result> {
        let bench_env = RunnerEnv::new();

        let pcm = if cfg!(feature = "pcm") {
            Some(pcm_stats)
        } else {
            None
        };

        let results: Vec<_> = bench_results.iter().map(|v| v.clone()).collect();

        let user_stats = f.additional_stats();

        BenchData {
            env: bench_env,
            thread_cnt,
            config: f.get_config().clone(),
            pcm,
            results,
            user_stats,
        }
    }

    fn print_loading(&self) {
        println!(
            "{}\n{}",
            "============================================================".red(),
            "Loading data...".cyan()
        );
    }

    fn print_running(&self, running_time: usize, name: &str, thread_cnt: usize) {
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
    pub fn start_bench<B: MultiThreadBench + 'static>(
        &self,
        f: &B,
    ) -> Vec<BenchData<B::Config, B::Result>> {
        let config = f.get_config();

        let running_time = match self.profile_time() {
            Some(t) => Duration::from_secs(t as u64),
            None => Duration::from_secs(config.bench_sec() as u64),
        };

        let mut results = Vec::new();

        for thread_cnt in config.thread().iter() {
            self.print_loading();

            f.load();

            self.print_running(
                running_time.as_secs() as usize,
                config.name(),
                *thread_cnt as usize,
            );

            let (bench_results, pcm_stats) = self.bench_one(*thread_cnt as usize, f);

            let result =
                self.extract_bench_results(f, *thread_cnt as usize, bench_results, pcm_stats);
            results.push(result);
        }

        results
    }
}
