use std::{
    fmt::Display,
    ops::{Add, AddAssign},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};

use serde::Serialize;

mod env;
mod metrics;
mod result;
mod runner;
pub use result::ShumaiResult;
pub use runner::run;
pub use shumai_config_impl::{config, ShumaiConfig};

pub mod __dep {
    pub use colored;
    pub use once_cell;
    pub use regex;
    pub use serde;
    pub use serde_json;
    pub use toml;
}

/// The context send to MultiBench::run()
pub struct Context<'a, C: BenchConfig> {
    pub thread_id: usize,
    pub thread_cnt: usize,
    pub config: &'a C,
    ready_thread: &'a AtomicU64,
    running: &'a AtomicBool,
}

impl<C: BenchConfig> Context<'_, C> {
    /// A barrier to ensure all threads start at exactly the same time,
    /// every run() should call context.wait_for_start() right after initialization or it will block forever.
    pub fn wait_for_start(&self) {
        self.ready_thread.fetch_add(1, Ordering::Relaxed);
        while !self.is_running() {
            std::hint::spin_loop();
        }
    }

    /// Main thread will let each bencher know whether to stop running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn thread_cnt(&self) -> usize {
        self.thread_cnt
    }

    pub fn thread_id(&self) -> usize {
        self.thread_id
    }
}

pub trait BenchResult:
    serde::Serialize + Default + AddAssign + Add<Output = Self> + Clone + Send + Sync + Display
{
    fn short_value(&self) -> usize;

    #[must_use]
    fn normalize_time(self, dur: &Duration) -> Self;
}

impl BenchResult for usize {
    fn short_value(&self) -> usize {
        *self
    }

    fn normalize_time(self, dur: &Duration) -> usize {
        ((self as f64) / dur.as_secs_f64()) as usize
    }
}

pub trait BenchConfig: Clone + Serialize + Send + Sync {
    fn name(&self) -> &String;
    fn thread(&self) -> &[usize];
    fn bench_sec(&self) -> usize;
}

/// The call chain of a MultiThreadBench:
/// load() -> run() [thread t1] -> run() [thread t2] -> ... -> cleanup()
pub trait ShumaiBench: Send + Sync {
    type Result: BenchResult;
    type Config: BenchConfig;

    /// The benchmark should init their code, load the necessary data and warmup the resources
    /// Note that the `load` will only be called once, no matter what `sample_size` is.
    fn load(&mut self) -> Option<serde_json::Value>;

    /// Run concurrent benchmark
    /// Inside this function should call context.wait_for_start() to notify the main thread;
    /// it also blocks current thread until every thread is ready (i.e. issued context.wait_for_start())
    fn run(&self, context: Context<Self::Config>) -> Self::Result;

    fn on_iteration_finished(&mut self, _cur_iter: usize) {}

    fn on_thread_finished(&mut self, _cur_thread: usize) {}

    /// clean up resources, if necessary
    fn cleanup(&mut self) -> Option<serde_json::Value>;
}
