use super::{Measure, Measurement};

pub(crate) struct FlamegraphMeasurement<'a> {
    guard: Option<pprof::ProfilerGuard<'a>>,
    report: Option<pprof::Report>,
}

impl<'a> FlamegraphMeasurement<'a> {
    pub(crate) fn new() -> Self {
        Self {
            guard: None,
            report: None,
        }
    }
}

impl<'a> Measurement for FlamegraphMeasurement<'a> {
    fn start(&mut self) {
        self.guard = Some(pprof::ProfilerGuard::new(199).unwrap());
    }

    fn stop(&mut self) {
        let guard = self.guard.take().unwrap();
        let report = guard.report().build().unwrap();
        self.report = Some(report);
    }

    fn result(&mut self) -> Measure {
        use chrono::{Datelike, Local, Timelike};
        use std::str::FromStr;

        let local_time = Local::now();
        let path = std::path::PathBuf::from_str(&format!(
            "target/benchmark/{}-{:02}-{:02}/{:02}-{:02}-{:02}.svg",
            local_time.year(),
            local_time.month(),
            local_time.day(),
            local_time.hour(),
            local_time.minute(),
            local_time.second()
        ))
        .unwrap();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let file = std::fs::File::create(path.clone()).unwrap();
        let report = self.report.take().unwrap();
        report.flamegraph(file).unwrap();
        Measure {
            name: "flamegraph".to_string(),
            value: serde_json::Value::String(path.to_str().unwrap().to_string()),
        }
    }
}

#[cfg(not(feature = "flamegraph"))]
pub(crate) fn flamegraph_of_func<F: FnOnce() -> R, R>(_config_name: &str, f: F) -> R {
    f()
}

#[cfg(feature = "flamegraph")]
pub(crate) fn flamegraph_of_func<F: FnOnce() -> R, R>(config_name: &str, f: F) -> R {
    use chrono::{Datelike, Local, Timelike};
    use std::str::FromStr;

    let guard = pprof::ProfilerGuard::new(200).unwrap();

    let rt = f();

    if let Ok(report) = guard.report().build() {
        let local_time = Local::now();
        let path = std::path::PathBuf::from_str(&format!(
            "target/benchmark/{}-{:02}-{:02}/{:02}-{:02}-{}.svg",
            local_time.year(),
            local_time.month(),
            local_time.day(),
            local_time.hour(),
            local_time.minute(),
            config_name
        ))
        .unwrap();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let file = std::fs::File::create(path).unwrap();
        report.flamegraph(file).unwrap();
    }

    rt
}
