use std::{
    fmt::Display,
    ops::{Add, AddAssign},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

mod pcm;

/// The context send to MultiBench::run()
pub struct BenchContext<'a> {
    pub thread_id: usize,
    pub thread_cnt: usize,
    ready_thread: &'a AtomicU64,
    running: &'a AtomicBool,
}

impl BenchContext<'_> {
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

pub trait BenchResult:
    serde::Serialize + Default + AddAssign + Add<Output = Self> + Clone + Send + Sync + Display
{
}

pub trait BenchConfig {
    fn name(&self) -> &String;
    fn thread(&self) -> &[u64];
    fn bench_sec(&self) -> usize;
}

pub trait MultiThreadBench: Send + Sync {
    type Result: BenchResult;
    type Config: BenchConfig;

    /// The benchmark should init their code, load the necessary data and warmup the resources
    /// Note that the `load` will only be called once, no matter what `sample_size` is.
    fn load(&self);

    /// run phase, run concurrent benchmark
    /// tid is not thread_id in unix, but the thread seq number, mostly from 0..thread_cnt
    /// it should also return an execution result, e.g., the # of total operations
    fn run(&self, context: BenchContext) -> Self::Result;

    /// clean up resources, if necessary
    fn cleanup(&self);

    fn get_config(&self) -> &Self::Config;

    /// Additional stats user want to include in the benchmark result,
    /// Such as the struct size/alignment, runtime configurations
    fn additional_stats(&self) -> Option<serde_json::Value> {
        None
    }
}
