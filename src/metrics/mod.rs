use serde::Serialize;

pub(crate) mod disk_io;
#[cfg(feature = "flamegraph")]
pub(crate) mod flamegraph;

#[cfg(feature = "pcm")]
pub(crate) mod pcm;

#[cfg(feature = "perf")]
pub(crate) mod perf;

#[derive(Debug, Clone, Serialize)]
pub struct Measure {
    name: String,
    value: serde_json::Value,
}

pub(crate) trait Measurement {
    fn start(&mut self) {}
    fn stop(&mut self) {}

    fn result(&mut self) -> Measure;
}
