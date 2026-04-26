use std::time::Instant;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct NetworkStats {
    pub interfaces: Vec<InterfaceStats>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InterfaceStats {
    pub name: String,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
}

/// Tracks previous byte counts to compute deltas.
#[derive(Default)]
pub struct NetworkTracker {
    prev_rx: Vec<u64>,
    prev_tx: Vec<u64>,
    prev_names: Vec<String>,
    prev_time: Option<Instant>,
}

#[cfg(windows)]
pub fn query_network_stats(tracker: &mut NetworkTracker) -> NetworkStats {
    use windows::Win32::NetworkManagement::IpHelper::GetIfTable2;
    use windows::Win32::NetworkManagement::IpHelper::FreeMibTable;

    let mut table_ptr = std::ptr::null_mut();
    let result = unsafe { GetIfTable2(&mut table_ptr) };

    if result.is_err() || table_ptr.is_null() {
        return NetworkStats::default();
    }

    let table = unsafe { &*table_ptr };
    let count = table.NumEntries as usize;
    let entries = unsafe {
        std::slice::from_raw_parts(table.Table.as_ptr(), count)
    };

    let now = Instant::now();
    let elapsed = tracker.prev_time.map(|t| now.duration_since(t).as_secs_f64()).unwrap_or(1.0);

    let mut interfaces = Vec::new();
    let mut curr_rx = Vec::new();
    let mut curr_tx = Vec::new();
    let mut curr_names = Vec::new();

    for entry in entries {
        // Skip loopback and tunnel interfaces
        let if_type = entry.Type;
        // 6 = ethernet, 71 = wifi
        if if_type != 6 && if_type != 71 {
            continue;
        }

        // Skip interfaces that are not up
        // OperStatus: 1 = up
        if entry.OperStatus.0 != 1 {
            continue;
        }

        let name = String::from_utf16_lossy(&entry.Description)
            .trim_end_matches('\0')
            .to_string();
        let rx = entry.InOctets;
        let tx = entry.OutOctets;

        // Find previous values for this interface by name
        let prev_idx = tracker.prev_names.iter().position(|n| *n == name);
        let (rx_rate, tx_rate) = if let Some(idx) = prev_idx {
            let drx = rx.saturating_sub(tracker.prev_rx[idx]) as f64;
            let dtx = tx.saturating_sub(tracker.prev_tx[idx]) as f64;
            (drx / elapsed, dtx / elapsed)
        } else {
            (0.0, 0.0)
        };

        curr_rx.push(rx);
        curr_tx.push(tx);
        curr_names.push(name.clone());

        interfaces.push(InterfaceStats {
            name,
            rx_bytes_per_sec: rx_rate,
            tx_bytes_per_sec: tx_rate,
        });
    }

    unsafe { FreeMibTable(table_ptr as _) };

    tracker.prev_rx = curr_rx;
    tracker.prev_tx = curr_tx;
    tracker.prev_names = curr_names;
    tracker.prev_time = Some(now);

    // Sort by traffic (busiest first) so .first() returns the active interface
    interfaces.sort_by(|a, b| {
        let ta = a.rx_bytes_per_sec + a.tx_bytes_per_sec;
        let tb = b.rx_bytes_per_sec + b.tx_bytes_per_sec;
        tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
    });

    NetworkStats { interfaces }
}

#[cfg(not(windows))]
pub fn query_network_stats(tracker: &mut NetworkTracker) -> NetworkStats {
    use sysinfo::Networks;

    let networks = Networks::new_with_refreshed_list();

    let now = Instant::now();
    let elapsed = tracker.prev_time.map(|t| now.duration_since(t).as_secs_f64()).unwrap_or(1.0);

    let mut interfaces = Vec::new();
    let mut curr_rx = Vec::new();
    let mut curr_tx = Vec::new();
    let mut curr_names = Vec::new();

    for (name, data) in &networks {
        // Skip loopback. We keep virtual interfaces (docker0, veth*, etc.) — the sort
        // by traffic below pushes idle ones to the bottom, and users on container
        // hosts often want them visible.
        if name == "lo" || name == "lo0" {
            continue;
        }
        // Apple-internal pseudo-interfaces carry no user-visible traffic; surfacing them
        // just clutters the panel. VPN tunnels (utun*) and Wi-Fi/ethernet are kept.
        #[cfg(target_os = "macos")]
        if matches!(
            name.as_str(),
            "gif0" | "stf0" | "awdl0" | "llw0" | "anpi0" | "anpi1" | "ap1"
        ) {
            continue;
        }

        let rx = data.total_received();
        let tx = data.total_transmitted();

        let prev_idx = tracker.prev_names.iter().position(|n| n == name);
        let (rx_rate, tx_rate) = if let Some(idx) = prev_idx {
            let drx = rx.saturating_sub(tracker.prev_rx[idx]) as f64;
            let dtx = tx.saturating_sub(tracker.prev_tx[idx]) as f64;
            (drx / elapsed, dtx / elapsed)
        } else {
            (0.0, 0.0)
        };

        curr_rx.push(rx);
        curr_tx.push(tx);
        curr_names.push(name.clone());

        interfaces.push(InterfaceStats {
            name: name.clone(),
            rx_bytes_per_sec: rx_rate,
            tx_bytes_per_sec: tx_rate,
        });
    }

    tracker.prev_rx = curr_rx;
    tracker.prev_tx = curr_tx;
    tracker.prev_names = curr_names;
    tracker.prev_time = Some(now);

    interfaces.sort_by(|a, b| {
        let ta = a.rx_bytes_per_sec + a.tx_bytes_per_sec;
        let tb = b.rx_bytes_per_sec + b.tx_bytes_per_sec;
        tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
    });

    NetworkStats { interfaces }
}
