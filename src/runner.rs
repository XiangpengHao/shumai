use serde::Serialize;

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
    ) -> (Vec<OneBenchResult<B>>, Vec<PcmStats>) {
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
                            thread_id: tid as u64,
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
                    .fold(B::Result::default(), |v, h| v + h.0.clone());

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
        bench_results: Vec<(B::Result, MetricResult)>,
        pcm_stats: Vec<PcmStats>,
    ) -> BenchResult<B::Config, B::Result> {
        let bench_env = BenchmarkEnv::new();

        let pcm = if cfg!(feature = "pcm") {
            Some(pcm_stats)
        } else {
            None
        };

        let results: Vec<_> = bench_results.iter().map(|v| v.0.clone()).collect();

        let user_stats = f.additional_stats();

        BenchResult {
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
    ) -> Vec<BenchResult<B::Config, B::Result>> {
        let config = f.get_config();

        let running_time = match self.profile_time() {
            Some(t) => Duration::from_secs(t as u64),
            None => Duration::from_secs(config.bench_sec() as u64),
        };

        let reuse_load = f.reuse_load();

        let mut results = Vec::new();

        if reuse_load {
            self.print_loading();

            f.load();

            for thread_cnt in config.thread().iter() {
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
        } else {
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
        }

        results
    }
}

#[derive(Debug, Serialize)]
pub struct RunnerEnv {
    os_release: String,
    rustc_version: String,
    hostname: String,
    cpu_num: usize,
    cpu_speed: u64,
}

impl Default for RunnerEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl RunnerEnv {
    pub fn new() -> Self {
        let cpu_num = sys_info::cpu_num().unwrap() as usize;
        let cpu_speed = sys_info::cpu_speed().unwrap();
        let hostname = sys_info::hostname().unwrap();
        let os_release = sys_info::os_release().unwrap();
        let rustc_ver = rustc_version::version().unwrap();
        let rustc_ver = format!(
            "{}.{}.{}",
            rustc_ver.major, rustc_ver.minor, rustc_ver.patch
        );
        Self {
            cpu_num,
            cpu_speed,
            hostname,
            os_release,
            rustc_version: rustc_ver,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BenchResult<T: Serialize + Clone + ConfigImpl, R: Serialize + Clone> {
    pub config: T,
    pub thread_cnt: usize,
    pub env: BenchmarkEnv,
    pub pcm: Option<Vec<PcmStats>>,
    pub results: Vec<R>,
    pub user_stats: Option<Value>,
}

impl<T: Serialize + Clone + ConfigImpl, R: Serialize + Clone> BenchResult<T, R> {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }

    pub fn write_json(&self) -> std::io::Result<()> {
        let local_time = Local::now();
        let file = format!(
            "target/benchmark/{}-{:02}-{:02}/{:02}-{:02}-{}-{}.json",
            local_time.year(),
            local_time.month(),
            local_time.day(),
            local_time.hour(),
            local_time.minute(),
            self.thread_cnt,
            self.config.name()
        );
        let path = Path::new(&file);
        let json_str = self.to_json();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, json_str)?;
        println!(
            "{}",
            format!("Benchmark results saved to file: {}", file).green()
        );
        Ok(())
    }
}
