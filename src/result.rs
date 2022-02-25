use chrono::{Datelike, Local, Timelike};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::PathBuf, str::FromStr};

use crate::{env::RunnerEnv, metrics::disk_io::DiskUsage, BenchConfig, ShumaiBench};

#[derive(Debug, Serialize, Deserialize)]
pub struct ShumaiResult<T: Serialize + Clone + BenchConfig, R: Serialize + Clone> {
    pub config: T,
    #[serde(rename = "load")]
    pub load_results: Option<Value>,
    #[serde(rename = "cleanup")]
    pub cleanup_results: Option<Value>,
    pub env: RunnerEnv,
    #[serde(rename = "run")]
    pub bench_results: Vec<ThreadResult<R>>,
}

impl<T: Serialize + Clone + BenchConfig, R: Serialize + Clone> ShumaiResult<T, R> {
    pub(crate) fn new(config: T, load_results: Option<Value>, env: RunnerEnv) -> Self {
        Self {
            config,
            load_results,
            cleanup_results: None,
            env,
            bench_results: Vec::new(),
        }
    }

    pub(crate) fn add_result(&mut self, result: ThreadResult<R>) {
        self.bench_results.push(result);
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }

    pub fn write_json(&self) -> std::io::Result<PathBuf> {
        use std::fs;

        let local_time = Local::now();
        let file = format!(
            "target/benchmark/{}-{:02}-{:02}/{:02}-{:02}-{}.json",
            local_time.year(),
            local_time.month(),
            local_time.day(),
            local_time.hour(),
            local_time.minute(),
            self.config.name()
        );
        let path = PathBuf::from_str(&file).unwrap();
        let json_str = self.to_json();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, json_str)?;
        println!(
            "{}",
            format!("Benchmark results saved to file: {}", file).green()
        );
        Ok(path)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThreadResult<R: Serialize + Clone> {
    pub thread_cnt: usize,
    pub results: Vec<R>,
    pub disk_usage: DiskUsage,
    #[cfg(feature = "pcm")]
    pub pcm: Vec<crate::metrics::pcm::PcmStats>,
    #[cfg(feature = "perf")]
    pub perf: crate::metrics::perf::PerfCounter,
}

#[derive(Debug, Clone)]
pub(crate) struct SampleResult<B: ShumaiBench> {
    pub(crate) result: B::Result,
    pub(crate) disk_usage: DiskUsage,
    #[cfg(feature = "perf")]
    pub(crate) perf: crate::metrics::perf::PerfCounter,
    #[cfg(feature = "pcm")]
    pub(crate) pcm: Vec<crate::metrics::pcm::PcmStats>,
}
