use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RunnerEnv {
    os_release: String,
    rustc_version: String,
    hostname: String,
    cpu_num: usize,
    cpu_speed: u64,
}

impl Default for RunnerEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl RunnerEnv {
    pub fn new() -> Self {
        let cpu_num = sys_info::cpu_num().unwrap() as usize;
        let cpu_speed = sys_info::cpu_speed().unwrap();
        let hostname = sys_info::hostname().unwrap();
        let os_release = sys_info::os_release().unwrap();
        let rustc_ver = rustc_version::version().unwrap();
        let rustc_ver = format!(
            "{}.{}.{}",
            rustc_ver.major, rustc_ver.minor, rustc_ver.patch
        );
        Self {
            cpu_num,
            cpu_speed,
            hostname,
            os_release,
            rustc_version: rustc_ver,
        }
    }
}
