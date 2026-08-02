//! Smoke tests for the `dofek-mock-plugin` fixture.
//!
//! The fixture exists to drive the plugin-runtime regression tests, but those
//! need runtime work that doesn't exist yet (a poll timeout that actually
//! fires, a stderr drainer, process-group containment). Until then these tests
//! pin the fixture's own behaviour, so it can't silently rot in the meantime.
//!
//! Modes covered here: default, `--legacy`, `--status-error`, `--crash-after`,
//! `--giant-line`, `--flood`, and `shutdown` handling. The modes that only make
//! sense against a non-blocking runtime — `--hang`, `--slow`, `--stderr-flood`,
//! `--fork-grandchild` — are exercised by the runtime tests, not here: with
//! today's blocking reader they would simply hang the suite.

#![cfg(feature = "mock-plugin")]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, Stdio};

const MOCK: &str = env!("CARGO_BIN_EXE_dofek-mock-plugin");

/// Spawn the fixture with `args`, returning the child and a line reader over
/// its stdout.
fn spawn(args: &[&str]) -> (Child, BufReader<ChildStdout>) {
    let mut child = Command::new(MOCK)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("mock plugin should spawn");
    let stdout = BufReader::new(child.stdout.take().unwrap());
    (child, stdout)
}

fn poll(child: &mut Child) {
    let stdin = child.stdin.as_mut().unwrap();
    writeln!(stdin, r#"{{"type":"poll","timestamp_ms":1,"processes":[]}}"#).unwrap();
    stdin.flush().unwrap();
}

fn shutdown(child: &mut Child) {
    let stdin = child.stdin.as_mut().unwrap();
    let _ = writeln!(stdin, r#"{{"type":"shutdown"}}"#);
    let _ = stdin.flush();
}

fn read_json(reader: &mut BufReader<ChildStdout>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("plugin should emit a line");
    serde_json::from_str(&line).expect("plugin output should be valid JSON")
}

#[test]
fn default_mode_speaks_the_protocol() {
    let (mut child, mut out) = spawn(&[]);

    // Poll 1 carries the manifest, as a real plugin does.
    poll(&mut child);
    let first = read_json(&mut out);
    assert_eq!(first["status"], "ok");
    assert_eq!(first["manifest"]["name"], "mock");
    assert_eq!(first["panels"][0]["id"], "mock");
    assert_eq!(first["panels"][0]["content"][0]["value"], "1");
    assert_eq!(first["metrics"][0]["value"], 1.0);
    assert!(first["process_annotations"][0]["pid"].is_number());

    // Poll 2 onward: no manifest, and the poll counter advances.
    poll(&mut child);
    let second = read_json(&mut out);
    assert!(second["manifest"].is_null(), "manifest should be sent once");
    assert_eq!(second["panels"][0]["content"][0]["value"], "2");

    shutdown(&mut child);
    let status = child.wait().unwrap();
    assert!(status.success(), "shutdown should exit 0, got {status}");
}

#[test]
fn legacy_mode_emits_only_the_v1_5_fields() {
    let (mut child, mut out) = spawn(&["--legacy"]);
    poll(&mut child);
    let resp = read_json(&mut out);

    assert_eq!(resp["status"], "ok");
    // Every optional payload field is omitted — this is the shape a plugin
    // written against the v1.5 protocol produces.
    for field in ["panels", "metrics", "process_annotations"] {
        assert!(resp[field].is_null(), "{field} should be absent in legacy mode");
    }

    shutdown(&mut child);
    assert!(child.wait().unwrap().success());
}

#[test]
fn status_error_mode_reports_error() {
    let (mut child, mut out) = spawn(&["--status-error", "--legacy"]);
    poll(&mut child);
    assert_eq!(read_json(&mut out)["status"], "error");

    shutdown(&mut child);
    assert!(child.wait().unwrap().success());
}

#[test]
fn crash_after_exits_nonzero_on_the_nth_poll() {
    let (mut child, mut out) = spawn(&["--crash-after", "2", "--legacy"]);

    poll(&mut child);
    assert_eq!(read_json(&mut out)["status"], "ok");
    poll(&mut child);
    assert_eq!(read_json(&mut out)["status"], "ok");

    let status = child.wait().unwrap();
    assert!(!status.success(), "should crash after 2 polls, got {status}");
}

#[test]
fn giant_line_emits_a_single_multi_megabyte_line() {
    let (mut child, mut out) = spawn(&["--giant-line"]);
    poll(&mut child);

    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    assert!(
        line.len() > 4 * 1024 * 1024,
        "expected a >4 MiB line, got {} bytes",
        line.len()
    );
    // Still one line, and still valid JSON — the point is size, not corruption.
    assert_eq!(line.matches('\n').count(), 1);
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["panels"][0]["id"], "giant");

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn flood_emits_many_responses_for_one_poll() {
    let (mut child, mut out) = spawn(&["--flood"]);
    poll(&mut child);

    for i in 0..1000 {
        let resp = read_json(&mut out);
        assert_eq!(resp["metrics"][0]["id"], format!("flood_{i}"));
    }

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn unknown_message_types_are_ignored() {
    let (mut child, mut out) = spawn(&["--legacy"]);

    let stdin = child.stdin.as_mut().unwrap();
    writeln!(stdin, r#"{{"type":"nonsense"}}"#).unwrap();
    writeln!(stdin, "not json at all").unwrap();
    writeln!(stdin).unwrap();
    stdin.flush().unwrap();

    // None of the above produced output; the following poll still works.
    poll(&mut child);
    assert_eq!(read_json(&mut out)["status"], "ok");

    shutdown(&mut child);
    assert!(child.wait().unwrap().success());
}
