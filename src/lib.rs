use std::{
    fmt::Display,
    ops::{Add, AddAssign},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use serde::Serialize;

mod env;
pub(crate) mod pcm;
mod result;
mod runner;
pub use runner::run;
pub use shumai_config_impl::{bench_config, ShumaiConfig};

pub mod __dep {
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
    /// Every MultiBench::run() should call context.wait_for_start() to let the main thread decide when to start running
    pub fn wait_for_start(&self) {
        self.ready_thread.fetch_add(1, Ordering::Relaxed);
        while !self.is_running() {}
    }

    /// Main thread will let each bencher know whether to stop running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn thread_cnt(&self) -> usize {
        self.thread_cnt
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
    fn thread(&self) -> &[u64];
    fn bench_sec(&self) -> usize;
}

/// The call chain of a MultiThreadBench:
/// load() -> run() [thread t1] -> run() [thread t2] -> ... -> cleanup()
pub trait MultiThreadBench: Send + Sync {
    type Result: BenchResult;
    type Config: BenchConfig;

    /// The benchmark should init their code, load the necessary data and warmup the resources
    /// Note that the `load` will only be called once, no matter what `sample_size` is.
    fn load(&self);

    /// run phase, run concurrent benchmark
    /// tid is not thread_id in unix, but the thread seq number, mostly from 0..thread_cnt
    /// it should also return an execution result, e.g., the # of total operations
    fn run(&self, context: BenchContext<Self::Config>) -> Self::Result;

    /// clean up resources, if necessary
    fn cleanup(&self);
}
