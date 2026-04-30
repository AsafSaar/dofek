//! dofek-net-ping: Plugin that samples TCP-connect latency to a remote host.
//!
//! Unlike dofek-ollama / dofek-docker, this plugin does periodic work *between*
//! polls: a background sampler thread opens a TCP connection every
//! `--interval-ms` and records the connect time (or a timeout) into a fixed-size
//! ring buffer. Each `poll` request reads the buffer and emits current / avg /
//! min / max / loss stats — the dashboard refreshes at its own cadence without
//! having to align to the ping interval.
//!
//! TCP connect time is used instead of ICMP because raw-socket ping needs root /
//! capabilities on most systems; TCP is what curl/HTTPS would see anyway and is
//! what most users actually care about.

use dofek_plugin_protocol::{Metric, Panel, PanelEntry, PluginManifest, PollResponse};
use std::collections::VecDeque;
use std::io::{self, BufRead, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SAMPLE_CAPACITY: usize = 60;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(2000);

/// One ping outcome. `None` means timeout / unreachable.
type Sample = Option<f64>;

struct PingState {
    samples: VecDeque<Sample>,
    last_error: Option<String>,
}

impl PingState {
    fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(SAMPLE_CAPACITY),
            last_error: None,
        }
    }

    fn push(&mut self, sample: Sample) {
        if self.samples.len() == SAMPLE_CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let host = parse_arg(&args, &["--host"]).unwrap_or_else(|| "1.1.1.1".to_string());
    let port: u16 = parse_arg(&args, &["--port"])
        .and_then(|s| s.parse().ok())
        .unwrap_or(443);
    let interval_ms: u64 = parse_arg(&args, &["--interval-ms", "--interval"])
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000)
        .max(100);

    let state = Arc::new(Mutex::new(PingState::new()));
    spawn_sampler(host.clone(), port, interval_ms, Arc::clone(&state));

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut first_response = true;

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(request) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        match request.get("type").and_then(|v| v.as_str()).unwrap_or("") {
            "shutdown" => break,
            "poll" => {
                let response = handle_poll(&host, port, &state, first_response);
                first_response = false;
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = writeln!(stdout, "{json}");
                    let _ = stdout.flush();
                }
            }
            _ => {}
        }
    }
}

fn parse_arg(args: &[String], names: &[&str]) -> Option<String> {
    let mut i = 1;
    while i < args.len() {
        if names.contains(&args[i].as_str()) && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

fn spawn_sampler(host: String, port: u16, interval_ms: u64, state: Arc<Mutex<PingState>>) {
    std::thread::spawn(move || {
        let interval = Duration::from_millis(interval_ms);
        loop {
            let started = Instant::now();
            let outcome = ping_once(&host, port);
            let elapsed = started.elapsed();

            {
                let mut s = state.lock().unwrap();
                match outcome {
                    Ok(latency_ms) => {
                        s.push(Some(latency_ms));
                        s.last_error = None;
                    }
                    Err(err) => {
                        s.push(None);
                        s.last_error = Some(err);
                    }
                }
            }

            // Sleep for the remainder of the interval; if the connect already
            // burned more than `interval`, fire the next sample immediately.
            if elapsed < interval {
                std::thread::sleep(interval - elapsed);
            }
        }
    });
}

fn ping_once(host: &str, port: u16) -> Result<f64, String> {
    let target = format!("{host}:{port}");
    let addrs: Vec<SocketAddr> = target
        .to_socket_addrs()
        .map_err(|e| format!("resolve: {e}"))?
        .collect();
    let addr = addrs.first().ok_or_else(|| "no addresses".to_string())?;

    let started = Instant::now();
    TcpStream::connect_timeout(addr, CONNECT_TIMEOUT).map_err(|e| e.to_string())?;
    Ok(started.elapsed().as_secs_f64() * 1000.0)
}

fn handle_poll(
    host: &str,
    port: u16,
    state: &Arc<Mutex<PingState>>,
    include_manifest: bool,
) -> PollResponse {
    let s = state.lock().unwrap();
    let samples = &s.samples;
    let last_error = s.last_error.clone();

    let mut response = PollResponse {
        status: "ok".to_string(),
        manifest: if include_manifest {
            Some(PluginManifest {
                name: "dofek-net-ping".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: "TCP-connect latency to a remote host".to_string(),
                author: "Dofek contributors".to_string(),
            })
        } else {
            None
        },
        ..PollResponse::default()
    };

    let mut content = vec![PanelEntry {
        key: "Host".to_string(),
        value: format!("{host}:{port}"),
        style: "dim".to_string(),
    }];

    if samples.is_empty() {
        content.push(PanelEntry {
            key: "Status".to_string(),
            value: "warming up".to_string(),
            style: "dim".to_string(),
        });
        response.panels.push(Panel {
            id: "net-ping".to_string(),
            label: "NET-PING".to_string(),
            content,
        });
        return response;
    }

    let total = samples.len();
    let oks: Vec<f64> = samples.iter().filter_map(|s| *s).collect();
    let lost = total - oks.len();
    let loss_pct = (lost as f64 / total as f64) * 100.0;
    let last = samples.back().copied().flatten();

    let (avg, min, max) = if oks.is_empty() {
        (None, None, None)
    } else {
        let sum: f64 = oks.iter().sum();
        let avg = sum / oks.len() as f64;
        let min = oks.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = oks.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (Some(avg), Some(min), Some(max))
    };

    match last {
        Some(ms) => content.push(PanelEntry {
            key: "Latency".to_string(),
            value: format!("{ms:.0} ms"),
            style: "accent".to_string(),
        }),
        None => content.push(PanelEntry {
            key: "Latency".to_string(),
            value: last_error
                .as_deref()
                .map(|e| format!("timeout ({e})"))
                .unwrap_or_else(|| "timeout".to_string()),
            style: "dim".to_string(),
        }),
    }

    if let (Some(avg), Some(min), Some(max)) = (avg, min, max) {
        content.push(PanelEntry {
            key: format!("Avg({total})"),
            value: format!("{avg:.0} ms"),
            style: "normal".to_string(),
        });
        content.push(PanelEntry {
            key: "Min/Max".to_string(),
            value: format!("{min:.0} / {max:.0} ms"),
            style: "dim".to_string(),
        });
    }

    content.push(PanelEntry {
        key: "Loss".to_string(),
        value: format!("{loss_pct:.0}% ({lost}/{total})"),
        style: if loss_pct > 0.0 { "accent" } else { "dim" }.to_string(),
    });

    if let Some(ms) = last {
        response.metrics.push(Metric {
            id: "netping.latency_ms".to_string(),
            label: "Ping".to_string(),
            value: ms,
            unit: "ms".to_string(),
        });
    }

    response.panels.push(Panel {
        id: "net-ping".to_string(),
        label: "NET-PING".to_string(),
        content,
    });

    response
}
