use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RunnerEnv {
    kernel_version: String,
    rustc_version: String,
    hostname: String,
    os_version: String,
    cpu_num: usize,
    physical_core_num: usize,
    total_memory: usize,
}

impl Default for RunnerEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl RunnerEnv {
    #[cfg(miri)]
    pub fn new() -> Self {
        Self {
            kernel_version: "".to_string(),
            rustc_version: "".to_string(),
            hostname: "".to_string(),
            os_version: "".to_string(),
            cpu_num: 0,
            physical_core_num: 0,
            total_memory: 0,
        }
    }

    #[cfg(not(miri))]
    pub fn new() -> Self {
        use sysinfo::SystemExt;

        let sys = sysinfo::System::new_all();

        let cpu_num = sys.physical_core_count().unwrap_or(0);
        let total_memory = sys.total_memory() as usize;
        let hostname = sys.host_name().unwrap();
        let kernel_version = sys.kernel_version().unwrap();
        let os_version = sys.os_version().unwrap();
        let rustc_ver = rustc_version::version().unwrap();
        let rustc_ver = format!(
            "{}.{}.{}",
            rustc_ver.major, rustc_ver.minor, rustc_ver.patch
        );

        Self {
            cpu_num,
            total_memory,
            physical_core_num: sys.physical_core_count().unwrap(),
            hostname,
            kernel_version,
            os_version,
            rustc_version: rustc_ver,
        }
    }
}
