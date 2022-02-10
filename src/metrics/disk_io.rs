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
    let process_id = sysinfo::get_current_pid().expect("unable to get pid");
    let process = sys_info.process(process_id);

    let rt = f();

    let disk_usage = match process {
        Some(p) => {
            let usage = p.disk_usage();
            DiskUsage {
                bytes_read: usage.read_bytes as usize,
                bytes_written: usage.written_bytes as usize,
            }
        }
        None => DiskUsage {
            bytes_read: 0,
            bytes_written: 0,
        },
    };

    (disk_usage, rt)
}
