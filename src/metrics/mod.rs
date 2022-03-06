use serde::Serialize;

pub(crate) mod disk_io;
pub(crate) mod flamegraph;
#[cfg(feature = "pcm")]
pub(crate) mod pcm;
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

// PerThreadMeasurement is mutual exclusive with Measurement, is there a way to do this?
pub(crate) trait PerThreadMeasurement: Sync {
    fn start(&self) {}
    fn stop(&self) {}
    fn result(&mut self) -> Measure;
}
