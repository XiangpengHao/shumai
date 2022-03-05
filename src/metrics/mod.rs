use serde::Serialize;

pub(crate) mod disk_io;
pub(crate) mod flamegraph;
#[cfg(feature = "pcm")]
pub(crate) mod pcm;
pub(crate) mod perf;

pub(crate) trait Measurement {
    fn start(&mut self) {}
    fn end(&mut self) {}
    fn per_thread_start(&self) {}
    fn per_thread_end(&self) {}

    fn result(&mut self) -> serde_json::Value;
}
