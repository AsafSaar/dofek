//! Cross-platform disk I/O rate tracker.
//!
//! Mirrors the `network::NetworkTracker` shape: keep cumulative read/write byte
//! counters per device by name, compute per-second deltas across calls.
//! Powered by `sysinfo::Disks`, so it works on Linux, macOS, and Windows
//! without per-OS branches.

use std::time::Instant;
use sysinfo::Disks;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DiskStats {
    pub devices: Vec<DiskDevice>,
    pub total_read_bytes_per_sec: f64,
    pub total_write_bytes_per_sec: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiskDevice {
    pub name: String,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

#[derive(Default)]
pub struct DiskTracker {
    prev_read: Vec<u64>,
    prev_write: Vec<u64>,
    prev_names: Vec<String>,
    prev_time: Option<Instant>,
}

pub fn query_disk_stats(tracker: &mut DiskTracker) -> DiskStats {
    let disks = Disks::new_with_refreshed_list();

    let now = Instant::now();
    let elapsed = tracker
        .prev_time
        .map(|t| now.duration_since(t).as_secs_f64())
        .unwrap_or(1.0);

    let mut devices = Vec::new();
    let mut curr_read = Vec::new();
    let mut curr_write = Vec::new();
    let mut curr_names = Vec::new();
    let mut total_read = 0.0_f64;
    let mut total_write = 0.0_f64;

    for disk in &disks {
        let name = disk.name().to_string_lossy().into_owned();

        // De-dupe partitions of the same physical device that sysinfo can list
        // separately; keep the first sighting per name.
        if curr_names.iter().any(|n: &String| n == &name) {
            continue;
        }

        let usage = disk.usage();
        let read = usage.total_read_bytes;
        let write = usage.total_written_bytes;

        let prev_idx = tracker.prev_names.iter().position(|n| *n == name);
        let (read_rate, write_rate) = if let Some(idx) = prev_idx {
            let dr = read.saturating_sub(tracker.prev_read[idx]) as f64;
            let dw = write.saturating_sub(tracker.prev_write[idx]) as f64;
            (dr / elapsed, dw / elapsed)
        } else {
            (0.0, 0.0)
        };

        curr_read.push(read);
        curr_write.push(write);
        curr_names.push(name.clone());

        total_read += read_rate;
        total_write += write_rate;

        devices.push(DiskDevice {
            name,
            read_bytes_per_sec: read_rate,
            write_bytes_per_sec: write_rate,
        });
    }

    tracker.prev_read = curr_read;
    tracker.prev_write = curr_write;
    tracker.prev_names = curr_names;
    tracker.prev_time = Some(now);

    devices.sort_by(|a, b| {
        let ta = a.read_bytes_per_sec + a.write_bytes_per_sec;
        let tb = b.read_bytes_per_sec + b.write_bytes_per_sec;
        tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
    });

    DiskStats {
        devices,
        total_read_bytes_per_sec: total_read,
        total_write_bytes_per_sec: total_write,
    }
}
