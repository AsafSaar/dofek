//! A deliberately misbehaving plugin, used as a test fixture.
//!
//! Every mode reproduces one real failure the plugin runtime has to survive.
//! Integration tests spawn it through `env!("CARGO_BIN_EXE_dofek-mock-plugin")`,
//! so Cargo builds it and hands the tests an exact path — no PATH lookup, no
//! fragile relative paths.
//!
//! Gated behind the default-off `mock-plugin` feature so it never ships in a
//! release build. Build it with `--features mock-plugin` (CI uses
//! `--all-features`).
//!
//! Protocol: newline-delimited JSON on stdin (`poll` / `shutdown`), one
//! `PollResponse` per line on stdout.

use std::io::{BufRead, Write};
use std::time::Duration;

use clap::Parser;
use dofek::plugin::protocol::{
    Metric, Panel, PanelEntry, PluginManifest, PollResponse, ProcessAnnotation,
};

#[derive(Parser, Debug)]
#[command(
    name = "dofek-mock-plugin",
    about = "Misbehaving plugin fixture for dofek's plugin-runtime tests"
)]
struct Args {
    /// Read the first poll, then never respond. Reproduces a wedged plugin
    /// stalling the collector (the non-functional `timeout_ms`).
    #[arg(long)]
    hang: bool,

    /// Delay this many milliseconds before each response.
    #[arg(long, value_name = "MS")]
    slow: Option<u64>,

    /// Exit non-zero after answering this many polls. Exercises respawn/backoff.
    #[arg(long, value_name = "N")]
    crash_after: Option<u32>,

    /// Emit many responses per poll. Exercises rate limiting and desync.
    #[arg(long)]
    flood: bool,

    /// Emit one ~4 MiB line. Exercises the bounded-read cap.
    #[arg(long)]
    giant_line: bool,

    /// Write 10 MiB to stderr before responding. Without a stderr drainer this
    /// deadlocks on the 64 KiB pipe buffer before stdout is ever written.
    #[arg(long)]
    stderr_flood: bool,

    /// Spawn a grandchild that outlives this process. Exercises Job Object
    /// (Windows) and process-group (Unix) containment.
    #[arg(long)]
    fork_grandchild: bool,

    /// Respond using only pre-v1.6 protocol fields. Guards backward
    /// compatibility as the response shape grows.
    #[arg(long)]
    legacy: bool,

    /// Report `status: "error"` on every poll.
    #[arg(long)]
    status_error: bool,

    /// Never echo the request's `seq`. Reproduces a pre-v1.6 plugin, so the
    /// runtime's first-reply-wins fallback gets exercised too.
    #[arg(long)]
    no_echo_seq: bool,

    /// Internal: sleep forever. Used as the target of `--fork-grandchild`.
    #[arg(long, hide = true)]
    grandchild: bool,
}

fn main() {
    let args = Args::parse();

    if args.grandchild {
        // Deliberately unkillable-by-parent-exit: the runtime must reap it.
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    }

    let mut grandchild_pid = None;
    if args.fork_grandchild {
        // Null stdio deliberately: if the grandchild inherited our stdout pipe
        // it would hold the write end open after we die, and the runtime would
        // never see EOF. That would make the containment test pass for the
        // wrong reason.
        grandchild_pid = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--grandchild")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()
            .map(|c| c.id());
    }

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut polls: u32 = 0;

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request = serde_json::from_str::<serde_json::Value>(line).ok();
        let msg_type = request
            .as_ref()
            .and_then(|v| v.get("type")?.as_str().map(str::to_owned))
            .unwrap_or_default();
        // Echoed back on every response unless `--no-echo-seq`, so the runtime
        // tests can drive both the seq-aware and the legacy pairing paths.
        let seq = if args.no_echo_seq || args.legacy {
            0
        } else {
            request
                .as_ref()
                .and_then(|v| v.get("seq")?.as_u64())
                .unwrap_or(0)
        };

        match msg_type.as_str() {
            "shutdown" => std::process::exit(0),
            "poll" => {}
            // Unknown message types are ignored, as a real plugin should.
            _ => continue,
        }

        polls += 1;

        if args.hang {
            // Never respond, but stay alive holding the pipe open — the worst
            // case for a caller that does a blocking read.
            loop {
                std::thread::sleep(Duration::from_secs(3600));
            }
        }

        if args.stderr_flood {
            let chunk = "x".repeat(1024);
            let mut stderr = std::io::stderr();
            for _ in 0..10 * 1024 {
                let _ = writeln!(stderr, "{chunk}");
            }
        }

        if let Some(ms) = args.slow {
            std::thread::sleep(Duration::from_millis(ms));
        }

        if args.giant_line {
            let mut resp = base_response(polls, seq, args.status_error);
            resp.panels = vec![Panel {
                id: "giant".into(),
                label: "GIANT".into(),
                content: vec![PanelEntry {
                    key: "blob".into(),
                    value: "A".repeat(4 * 1024 * 1024),
                    style: "normal".into(),
                }],
            }];
            emit(&mut stdout, &resp);
        } else if args.flood {
            for i in 0..1000 {
                let mut resp = base_response(polls, seq, args.status_error);
                resp.metrics = vec![Metric {
                    id: format!("flood_{i}"),
                    label: format!("F{i}"),
                    value: i as f64,
                    unit: String::new(),
                }];
                emit(&mut stdout, &resp);
            }
        } else {
            let mut resp = base_response(polls, seq, args.status_error);
            if !args.legacy {
                resp.panels = vec![Panel {
                    id: "mock".into(),
                    label: "MOCK".into(),
                    content: vec![
                        PanelEntry {
                            key: "polls".into(),
                            value: polls.to_string(),
                            style: "normal".into(),
                        },
                        PanelEntry {
                            key: "grandchild".into(),
                            value: grandchild_pid
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| "-".into()),
                            style: "normal".into(),
                        },
                    ],
                }];
                resp.metrics = vec![Metric {
                    id: "mock_polls".into(),
                    label: "POLLS".into(),
                    value: polls as f64,
                    unit: String::new(),
                }];
                resp.process_annotations = vec![ProcessAnnotation {
                    pid: std::process::id(),
                    label: Some("mock".into()),
                    category: None,
                    ai_state: None,
                }];
            }
            emit(&mut stdout, &resp);
        }

        if let Some(n) = args.crash_after
            && polls >= n
        {
            std::process::exit(1);
        }
    }
}

/// Response skeleton: the manifest goes out on the first poll only, matching
/// what a well-behaved plugin does.
fn base_response(polls: u32, seq: u64, status_error: bool) -> PollResponse {
    PollResponse {
        status: if status_error { "error".into() } else { "ok".into() },
        seq,
        manifest: (polls == 1).then(|| PluginManifest {
            name: "mock".into(),
            version: "0.0.0".into(),
            description: "test fixture".into(),
            author: "dofek".into(),
        }),
        ..Default::default()
    }
}

fn emit(stdout: &mut std::io::Stdout, resp: &PollResponse) {
    let json = serde_json::to_string(resp).expect("PollResponse always serializes");
    // Ignore write errors: a test that drops the pipe mid-run is a valid case.
    let _ = writeln!(stdout, "{json}");
    let _ = stdout.flush();
}
