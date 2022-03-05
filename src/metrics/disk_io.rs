use sysinfo::{ProcessExt, SystemExt};

use super::Measurement;

pub(crate) struct DiskIoMeasurement {
    sys_info: sysinfo::System,
    result: Option<DiskUsage>,
}

impl DiskIoMeasurement {
    pub(crate) fn new() -> Self {
        Self {
            sys_info: sysinfo::System::new(),
            result: None,
        }
    }
}

impl Measurement for DiskIoMeasurement {
    fn start(&mut self) {
        self.sys_info.refresh_disks();
        self.sys_info.refresh_processes();
    }

    fn end(&mut self) {
        let process_id = sysinfo::get_current_pid().expect("unable to get pid");
        let process = self
            .sys_info
            .process(process_id)
            .expect("unable to get process");
        let disk_usage = process.disk_usage();

        self.result = Some(DiskUsage {
            bytes_read: disk_usage.read_bytes as usize,
            bytes_written: disk_usage.written_bytes as usize,
        })
    }

    fn result(&mut self) -> serde_json::Value {
        match &self.result {
            Some(result) => serde_json::to_value(result).unwrap(),
            None => serde_json::Value::Null,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiskUsage {
    pub bytes_read: usize,
    pub bytes_written: usize,
}

pub(crate) fn disk_io_of_func<F: FnOnce() -> R, R>(f: F) -> (DiskUsage, R) {
    // Start our disk counter
    let mut sys_info = sysinfo::System::new();
    sys_info.refresh_disks();
    sys_info.refresh_processes();
    let process_id = sysinfo::get_current_pid().expect("unable to get pid");
    let process = sys_info.process(process_id).expect("unable to get process");

    let rt = f();

    let disk_usage = process.disk_usage();

    (
        DiskUsage {
            bytes_read: disk_usage.read_bytes as usize,
            bytes_written: disk_usage.written_bytes as usize,
        },
        rt,
    )
}
