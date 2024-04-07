use chrono::{Datelike, Local, Timelike};
use colored::Colorize;
use serde::Serialize;
use serde_json::Value;
use std::{path::PathBuf, str::FromStr, time::Duration};

use crate::{env::RunnerEnv, metrics::Measure, BenchConfig};

#[derive(Debug, Serialize)]
pub struct LoadResults {
    pub time_elapsed: Duration,
    pub user_metrics: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct ShumaiResult<T: Serialize + Clone + BenchConfig, R: Serialize + Clone> {
    pub config: T,
    #[serde(rename = "load")]
    pub load_results: LoadResults,
    #[serde(rename = "cleanup")]
    pub cleanup_results: Option<Value>,
    pub env: RunnerEnv,
    #[serde(rename = "run")]
    pub bench_results: Vec<ThreadResult<R>>,
}

impl<T: Serialize + Clone + BenchConfig, R: Serialize + Clone> ShumaiResult<T, R> {
    pub(crate) fn new(config: T, load_results: LoadResults, env: RunnerEnv) -> Self {
        Self {
            config,
            load_results,
            cleanup_results: None,
            env,
            bench_results: Vec::new(),
        }
    }

    pub(crate) fn add(&mut self, result: ThreadResult<R>) {
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

#[derive(Debug, Serialize)]
pub struct ThreadResult<R: Serialize> {
    pub thread_cnt: usize,
    pub iterations: Vec<BenchValue<R>>,
    pub on_thread_finished: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchValue<R: Serialize> {
    pub(crate) result: R,
    pub(crate) measurements: Vec<Measure>,
}
