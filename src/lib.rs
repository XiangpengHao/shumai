use std::{
    fmt::Display,
    ops::{Add, AddAssign},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use serde::Serialize;

mod counters;
mod env;
mod result;
mod runner;
pub use result::ShumaiResult;
pub use runner::run;
pub use shumai_config_impl::{bench_config, ShumaiConfig};

pub mod __dep {
    pub use colored;
    pub use once_cell;
    pub use regex;
    pub use serde;
    pub use serde_json;
    pub use toml;
}

/// The context send to MultiBench::run()
pub struct BenchContext<'a, C: BenchConfig> {
    pub thread_id: usize,
    pub thread_cnt: usize,
    pub config: &'a C,
    ready_thread: &'a AtomicU64,
    running: &'a AtomicBool,
}

impl<C: BenchConfig> BenchContext<'_, C> {
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

unsafe impl<C: BenchConfig> Send for BenchContext<'_, C> {}
unsafe impl<C: BenchConfig> Sync for BenchContext<'_, C> {}

pub trait BenchResult:
    serde::Serialize + Default + AddAssign + Add<Output = Self> + Clone + Send + Sync + Display
{
}

impl BenchResult for usize {}

pub trait BenchConfig: Clone + Serialize {
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
    fn load(&self) -> Option<serde_json::Value>;

    /// Run concurrent benchmark
    /// Inside this function should call context.wait_for_start() to notify the main thread;
    /// it also blocks current thread until every thread is ready (i.e. issued context.wait_for_start())
    fn run(&self, context: BenchContext<Self::Config>) -> Self::Result;

    /// clean up resources, if necessary
    fn cleanup(&self) -> Option<serde_json::Value>;
}
