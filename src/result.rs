use chrono::{Datelike, Local, Timelike};
use colored::Colorize;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

use crate::{env::RunnerEnv, pcm::PcmStats, BenchConfig};

#[derive(Debug, Serialize)]
pub struct BenchData<T: Serialize + Clone + BenchConfig, R: Serialize + Clone> {
    pub config: T,
    pub thread_cnt: usize,
    pub env: RunnerEnv,
    pub pcm: Option<Vec<PcmStats>>,
    pub results: Vec<R>,
    pub user_stats: Option<Value>,
}

impl<T: Serialize + Clone + BenchConfig, R: Serialize + Clone> BenchData<T, R> {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }

    pub fn write_json(&self) -> std::io::Result<()> {
        use std::fs;

        let local_time = Local::now();
        let file = format!(
            "target/benchmark/{}-{:02}-{:02}/{:02}-{:02}-{}-{}.json",
            local_time.year(),
            local_time.month(),
            local_time.day(),
            local_time.hour(),
            local_time.minute(),
            self.thread_cnt,
            self.config.name()
        );
        let path = Path::new(&file);
        let json_str = self.to_json();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, json_str)?;
        println!(
            "{}",
            format!("Benchmark results saved to file: {}", file).green()
        );
        Ok(())
    }
}
