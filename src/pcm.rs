use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct PcmStats {
    pm_read: u64,
    pm_write: u64,
    dram_read: u64,
    dram_write: u64,
    l3_hit: u64,
    l3_miss: u64,
    /// TODO: this is not accurate, currently it refers to the UPI link 0 on socket 0
    upi_in_util: f64,
    upi_out_util: f64,
}

fn extract_u64(val: &Value) -> Option<u64> {
    if let Value::Number(n) = val {
        n.as_u64()
    } else {
        None
    }
}

fn extract_f64(val: &Value) -> Option<f64> {
    if let Value::Number(n) = val {
        n.as_f64()
    } else {
        None
    }
}

impl PcmStats {
    pub(crate) fn from_request() -> PcmStats {
        let body = ureq::get("http://localhost:9738/persecond")
            .set("Accept", "application/json")
            .call()
            .unwrap()
            .into_json()
            .expect(
                "Failed to send request to localhost:9738, did you start the pcm-sensor-server?",
            );

        PcmStats::from_json(&body)
    }

    pub(crate) fn from_json(val: &Value) -> PcmStats {
        let socket0 = &val["Sockets"][0];

        let l3_miss = &socket0["Core Aggregate"]["Core Counters"]["L3 Cache Misses"];
        let l3_hit = &socket0["Core Aggregate"]["Core Counters"]["L3 Cache Hits"];

        let pm_read = &socket0["Uncore"]["Uncore Counters"]["Persistent Memory Reads"];
        let pm_write = &socket0["Uncore"]["Uncore Counters"]["Persistent Memory Writes"];
        let dram_read = &socket0["Uncore"]["Uncore Counters"]["DRAM Writes"];
        let dram_write = &socket0["Uncore"]["Uncore Counters"]["DRAM Writes"];

        let upi0 = &val["QPI/UPI Links"]["QPI Counters Socket 0"];
        let upi_in_util = &upi0["Utilization Incoming Data Traffic On Link 0"];
        let upi_out_util = &upi0["Utilization Outgoing Data And Non-Data Traffic On Link 0"];

        let l3_miss = extract_u64(l3_miss).unwrap_or(0);
        let l3_hit = extract_u64(l3_hit).unwrap_or(0);
        let pm_read = extract_u64(pm_read).unwrap_or(0);
        let pm_write = extract_u64(pm_write).unwrap_or(0);
        let dram_read = extract_u64(dram_read).unwrap_or(0);
        let dram_write = extract_u64(dram_write).unwrap_or(0);

        // Single socket server don't have following metrics
        let upi_in_util = extract_f64(upi_in_util).unwrap_or(0.0);
        let upi_out_util = extract_f64(upi_out_util).unwrap_or(0.0);

        PcmStats {
            pm_read,
            pm_write,
            dram_read,
            dram_write,
            l3_hit,
            l3_miss,
            upi_in_util,
            upi_out_util,
        }
    }
}
