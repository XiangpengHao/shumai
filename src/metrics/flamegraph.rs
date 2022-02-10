use std::str::FromStr;

use chrono::{Datelike, Local, Timelike};

pub(crate) fn flamegraph_of_func<F: FnOnce() -> R, R>(config_name: &str, f: F) -> R {
    // if flamegraph feature is enabled
    let guard = if cfg!(feature = "flamegraph") {
        Some(pprof::ProfilerGuard::new(200).unwrap())
    } else {
        None
    };

    let rt = f();

    // save the flamegraph
    if let Some(guard) = guard {
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
    }

    rt
}
