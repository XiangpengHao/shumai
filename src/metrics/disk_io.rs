use sysinfo::{ProcessExt, SystemExt};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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
