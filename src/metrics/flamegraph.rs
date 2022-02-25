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
