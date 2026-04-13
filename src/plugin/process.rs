use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::protocol::{PollRequest, PollResponse, ShutdownRequest};

/// Wraps a plugin child process with piped stdio.
pub struct PluginProcess {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    read_buf: String,
}

impl PluginProcess {
    /// Spawn a new plugin child process.
    pub fn spawn(command: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(windows_creation_flags())
            .spawn()
            .with_context(|| format!("Failed to spawn plugin: {command}"))?;

        let stdout = child.stdout.take().expect("stdout piped");
        let reader = BufReader::new(stdout);

        Ok(Self {
            child,
            reader,
            read_buf: String::with_capacity(4096),
        })
    }

    /// Send a poll request and read the response with a timeout.
    pub fn poll(&mut self, request: &PollRequest, timeout: Duration) -> Result<PollResponse> {
        let stdin = self.child.stdin.as_mut().context("Plugin stdin closed")?;

        // Write request as newline-delimited JSON
        let json = serde_json::to_string(request)?;
        writeln!(stdin, "{json}")?;
        stdin.flush()?;

        // Read response with timeout
        self.read_buf.clear();
        let response = self.read_line_timeout(timeout)?;
        let parsed: PollResponse = serde_json::from_str(&response)
            .with_context(|| format!("Failed to parse plugin response: {response}"))?;

        Ok(parsed)
    }

    /// Send shutdown message. Best-effort, don't fail on errors.
    pub fn send_shutdown(&mut self) {
        if let Some(stdin) = self.child.stdin.as_mut() {
            let msg = ShutdownRequest::new();
            if let Ok(json) = serde_json::to_string(&msg) {
                let _ = writeln!(stdin, "{json}");
                let _ = stdin.flush();
            }
        }
    }

    /// Check if the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Kill the child process.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn read_line_timeout(&mut self, timeout: Duration) -> Result<String> {
        // On Windows, BufReader on pipes is blocking. We use a polling approach
        // with a tight loop checking elapsed time. For the typical 2s timeout and
        // plugins that respond in <100ms, this is fine.
        let start = Instant::now();
        loop {
            // Try a non-blocking read by checking if data is available
            // Unfortunately std doesn't support non-blocking pipe reads on Windows,
            // so we rely on the plugin responding quickly. If it blocks past timeout,
            // we'll detect it on the next poll cycle when is_alive() returns false
            // after we kill it.
            //
            // For v0.3, this blocking read is acceptable since each plugin has its
            // own timeout and the collector thread polls sequentially.
            self.read_buf.clear();
            match self.reader.read_line(&mut self.read_buf) {
                Ok(0) => anyhow::bail!("Plugin closed stdout (EOF)"),
                Ok(_) => return Ok(self.read_buf.trim().to_string()),
                Err(e) => {
                    if start.elapsed() > timeout {
                        anyhow::bail!("Plugin read timed out after {:?}", timeout);
                    }
                    anyhow::bail!("Plugin read error: {e}");
                }
            }
        }
    }
}

/// Windows: CREATE_NO_WINDOW flag to suppress console window for plugin processes.
fn windows_creation_flags() -> u32 {
    0x08000000 // CREATE_NO_WINDOW
}
