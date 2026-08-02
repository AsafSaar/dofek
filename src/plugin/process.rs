//! One plugin child process: spawn, contained; write requests; receive
//! response lines off a dedicated reader thread; terminate the whole process
//! group.
//!
//! This deliberately knows nothing about polling cadence, timeouts, backoff or
//! health — that all lives in [`super::worker`]. What it owns is the OS-level
//! contract:
//!
//! * **Reads never block the caller.** A reader thread owns `stdout` and pushes
//!   whole lines into a bounded channel. The old `read_line_timeout` did one
//!   *blocking* `read_line` and only consulted the clock in the `Err` branch,
//!   so `timeout_ms` never fired and one wedged plugin froze the collector —
//!   and with it every metric in both UIs.
//! * **Reads are bounded.** A line over [`MAX_LINE_BYTES`] disconnects the
//!   plugin instead of growing a `String` until the process dies. Resyncing
//!   after a partial line is not attempted: we cannot tell a truncated giant
//!   from a legitimate line that happens to follow it.
//! * **stderr is drained.** It was previously piped and never read, so any
//!   plugin logging more than the 64 KiB pipe buffer deadlocked itself before
//!   it could answer on stdout — and `plugins/README.md` promised otherwise.
//! * **Children are contained.** A Job Object on Windows and a session/process
//!   group on Unix mean a plugin's own children die with it. This is what
//!   `CLAUDE.md` already claimed and the code did not do.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

/// Hard cap on one response line. Generous — the biggest legitimate response
/// (512 annotations at the sanitizer's caps) is a few tens of KiB — and small
/// enough that a plugin emitting an endless line is cut off long before it
/// matters.
pub const MAX_LINE_BYTES: usize = 256 * 1024;

/// Reader-to-supervisor queue depth. Small on purpose: a plugin that answers
/// faster than it is asked applies backpressure to its own reader thread
/// rather than accumulating in our heap. Also the hard ceiling on how many
/// unsolicited lines one drain can ever see — see `worker::FLOOD_LINES`.
pub const RESPONSE_QUEUE: usize = 4;

/// How long to wait for a child to exit after being told to, before escalating
/// to signals. Kept short deliberately: every plugin now tears down on its own
/// thread, so this is a per-plugin worst case rather than a sum, and quitting
/// dofek should feel instant even with a plugin that ignores us.
const TERM_GRACE: Duration = Duration::from_millis(150);

/// How long a process group gets between `SIGTERM` and `SIGKILL`.
const GROUP_TERM_GRACE: Duration = Duration::from_millis(100);

/// What the reader thread hands back.
#[derive(Debug)]
pub enum ReadEvent {
    /// One complete response line, newline stripped.
    Line(String),
    /// The reader stopped for good: EOF, an I/O error, or an over-long line.
    /// The supervisor treats every variant the same way — disconnect and let
    /// backoff decide when to try again — but the reason is worth logging.
    Closed(String),
}

/// A spawned plugin with its stdio wired up.
pub struct PluginProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    responses: Receiver<ReadEvent>,
    /// Unix: the child's process group, captured at spawn and confirmed to be
    /// led by the child itself. `None` if `setsid` somehow didn't take, in
    /// which case we only ever signal the child — signalling a group we don't
    /// own could hit dofek itself.
    #[cfg(unix)]
    group: Option<i32>,
    /// Set once the child has been signalled and reaped.
    ///
    /// Load-bearing: a reaped pid is free for the OS to reissue, and a
    /// process-group signal sent after that could land on something unrelated.
    /// Nothing in this module reaps outside [`PluginProcess::kill`], and this
    /// flag makes that call idempotent.
    reaped: bool,
    /// Kept alive for the lifetime of the process: dropping it kills every
    /// process still in the job. Unused on other platforms.
    #[cfg(windows)]
    _job: job::Job,
}

impl PluginProcess {
    /// Spawn a plugin child process.
    ///
    /// `command` must be an absolute path to a regular file, or the bare name
    /// of a plugin installed in the managed directory
    /// (`<config_dir>/dofek/plugins/`). Anything else is rejected here rather
    /// than handed to `Command::new`, which would fall back to `PATH` — and,
    /// on Windows, to the current working directory.
    ///
    /// `label` only names the process in log lines.
    pub fn spawn(command: &str, args: &[String], label: &str) -> Result<Self> {
        let resolved = super::store::resolve_command(command)?;
        Self::spawn_resolved(&resolved, args, label)
    }

    /// [`PluginProcess::spawn`] against an already-resolved path. The store
    /// uses this to probe a binary it has just written, before there is a
    /// `plugins.toml` entry to resolve through.
    pub fn spawn_resolved(resolved: &std::path::Path, args: &[String], label: &str) -> Result<Self> {
        let mut cmd = Command::new(resolved);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        // Unix: put the child in its own session, so it becomes the leader of
        // a brand-new process group whose id equals its pid. Anything it forks
        // inherits that group, which is what makes `killpg` reap grandchildren
        // instead of leaving them orphaned on the machine.
        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                nix::unistd::setsid()
                    .map(|_| ())
                    .map_err(|e| std::io::Error::from_raw_os_error(e as i32))
            });
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn plugin: {}", resolved.display()))?;

        // Windows: contain the child in a Job Object that kills everything
        // inside it when the last handle closes.
        #[cfg(windows)]
        let _job = job::Job::containing(&child);

        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");
        let stdin = child.stdin.take();

        // Confirm `setsid` actually took, once, while the child is definitely
        // alive. Everything after this point trusts the captured value rather
        // than re-querying — see the `reaped` field.
        #[cfg(unix)]
        let group = {
            let pid = nix::unistd::Pid::from_raw(child.id() as i32);
            match nix::unistd::getpgid(Some(pid)) {
                Ok(g) if g == pid => Some(g.as_raw()),
                _ => {
                    log::warn!(
                        "plugin '{label}' did not become its own process group leader; \
                         its children will not be contained"
                    );
                    None
                }
            }
        };

        let (tx, responses) = sync_channel(RESPONSE_QUEUE);
        spawn_reader(stdout, tx, label);
        spawn_stderr_drainer(stderr, label);

        Ok(Self {
            child,
            stdin,
            responses,
            #[cfg(unix)]
            group,
            reaped: false,
            #[cfg(windows)]
            _job,
        })
    }

    /// The child's OS process id.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Write one newline-terminated JSON message to the child's stdin.
    ///
    /// This is the only place the caller can block, and only for as long as
    /// the OS pipe buffer is full — which for a request that is at most a few
    /// hundred KiB against a 64 KiB pipe means "until the plugin reads". A
    /// plugin that never reads its stdin is caught the same way one that never
    /// writes is: the response times out and the supervisor restarts it.
    pub fn send_line(&mut self, json: &str) -> Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .context("plugin stdin is closed")?;
        writeln!(stdin, "{json}")?;
        stdin.flush()?;
        Ok(())
    }

    /// Wait up to `timeout` for the next event from the reader thread.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<ReadEvent, RecvTimeoutError> {
        self.responses.recv_timeout(timeout)
    }

    /// Take an already-queued event without waiting.
    pub fn try_recv(&self) -> Result<ReadEvent, TryRecvError> {
        self.responses.try_recv()
    }

    /// Send the shutdown message. Best-effort: a child that has already exited
    /// or closed stdin is not an error at this point.
    pub fn send_shutdown(&mut self) {
        let msg = super::protocol::ShutdownRequest::new();
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = self.send_line(&json);
        }
    }

    /// Ask the plugin to exit, then tear down its whole process group.
    ///
    /// Closing stdin first matters: a plugin whose read loop is a plain
    /// `for line in stdin.lock().lines()` — which is what the protocol docs
    /// show, and what all three first-party plugins do — exits on EOF even if
    /// it ignores the shutdown message entirely.
    ///
    /// The group teardown runs **even when the plugin exited cleanly**. A
    /// plugin that quits leaving background helpers behind is exactly the
    /// orphan case containment exists to prevent, and "the plugin exited"
    /// says nothing about what it started.
    pub fn shutdown_and_kill(&mut self) {
        if self.reaped {
            return;
        }
        self.send_shutdown();
        self.stdin = None; // drop → EOF on the child's stdin
        // A flat grace rather than a polled one. The obvious optimisation —
        // `try_wait` in a loop and stop early — reaps the child, which frees
        // its pid, which is the group id we are about to signal. Racing a pid
        // reissue to send SIGKILL to an unrelated process group is not a
        // trade worth 150 ms, especially now that every plugin does this
        // concurrently on its own thread.
        thread::sleep(TERM_GRACE);
        self.kill();
    }

    /// Terminate the child and everything it spawned. Idempotent.
    pub fn kill(&mut self) {
        if self.reaped {
            return;
        }
        self.stdin = None;
        self.signal_group(GroupSignal::Term);
        thread::sleep(GROUP_TERM_GRACE);
        self.signal_group(GroupSignal::Kill);
        // Reap last, so every signal above went out while the pid — and
        // therefore the process group id — was still ours.
        let _ = self.child.wait();
        self.reaped = true;
    }

    /// Unix: signal the child's whole process group so grandchildren die too.
    ///
    /// Falls back to signalling the child alone if `setsid` didn't take. That
    /// check is not paranoia: `killpg` against our *own* group would signal
    /// dofek itself. A leaked grandchild is bad; killing the monitor is worse.
    #[cfg(unix)]
    fn signal_group(&mut self, sig: GroupSignal) {
        use nix::sys::signal::{Signal, kill, killpg};
        use nix::unistd::Pid;

        let sig = match sig {
            GroupSignal::Term => Signal::SIGTERM,
            GroupSignal::Kill => Signal::SIGKILL,
        };
        match self.group {
            Some(pgid) => {
                let _ = killpg(Pid::from_raw(pgid), sig);
            }
            None => {
                let _ = kill(Pid::from_raw(self.child.id() as i32), sig);
            }
        }
    }

    /// Windows: the Job Object does the containment — closing its handle when
    /// this `PluginProcess` drops terminates every process still inside. All
    /// that is needed here is to stop the direct child promptly.
    #[cfg(not(unix))]
    fn signal_group(&mut self, sig: GroupSignal) {
        if matches!(sig, GroupSignal::Kill) {
            let _ = self.child.kill();
        }
    }
}

/// Which stage of termination to signal. Named rather than passing a raw
/// signal so the Windows path doesn't have to pretend POSIX signals exist.
#[derive(Clone, Copy)]
enum GroupSignal {
    /// "Please exit" — the plugin and its helpers get [`GROUP_TERM_GRACE`].
    Term,
    /// "Now."
    Kill,
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        // A worker that panics, or a manager dropped without `shutdown()`,
        // must not leave a plugin (or its grandchildren) running. No-ops if
        // the process was already torn down.
        self.kill();
    }
}

/// Windows: suppress the console window a plugin would otherwise flash.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Read whole lines off the child's stdout and push them into `tx`.
///
/// Sacrificial by design: it dies when the child does, and the supervisor
/// never joins it. Blocking forever in `read_line` is fine here precisely
/// because nothing else is waiting on this thread.
fn spawn_reader(stdout: ChildStdout, tx: SyncSender<ReadEvent>, label: &str) {
    let label = label.to_string();
    let _ = thread::Builder::new()
        .name(format!("plugin-out:{label}"))
        .spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut buf = String::new();
            loop {
                buf.clear();
                // `take` is rebuilt every iteration so the cap applies per
                // line, not per connection.
                let read = reader.by_ref().take(MAX_LINE_BYTES as u64).read_line(&mut buf);
                let event = match read {
                    Ok(0) => ReadEvent::Closed("plugin closed stdout (EOF)".into()),
                    Ok(n) if n >= MAX_LINE_BYTES && !buf.ends_with('\n') => ReadEvent::Closed(
                        format!("plugin response line exceeded {MAX_LINE_BYTES} bytes"),
                    ),
                    // An over-long line whose tail happens to be invalid UTF-8
                    // surfaces here rather than as a length overrun; either way
                    // the connection is unusable.
                    Ok(_) => ReadEvent::Line(buf.trim().to_string()),
                    Err(e) => ReadEvent::Closed(format!("plugin read error: {e}")),
                };
                let terminal = matches!(event, ReadEvent::Closed(_));
                // A full queue means the supervisor is behind; blocking here is
                // the backpressure. A disconnected one means the plugin is
                // already gone.
                if tx.send(event).is_err() || terminal {
                    return;
                }
            }
        });
}

/// Drain the child's stderr so it can never fill its pipe buffer, surfacing it
/// as rate-limited debug logging.
///
/// Rate limiting matters as much as draining: a plugin logging every request
/// would otherwise dominate `--debug` output and, through `env_logger`, cost
/// real time on the process that is trying to monitor the machine.
fn spawn_stderr_drainer(stderr: ChildStderr, label: &str) {
    /// At most one stderr line per plugin per this interval reaches the log.
    const LOG_INTERVAL: Duration = Duration::from_secs(1);
    /// Read cap per line — stderr is diagnostics, not data.
    const MAX_STDERR_LINE: u64 = 8 * 1024;

    let label = label.to_string();
    let _ = thread::Builder::new()
        .name(format!("plugin-err:{label}"))
        .spawn(move || {
            let mut reader = BufReader::new(stderr);
            // Bytes, not `String`: a plugin's stderr is not required to be
            // valid UTF-8 and a decode error must not stop the draining.
            let mut buf: Vec<u8> = Vec::new();
            let mut last_log: Option<Instant> = None;
            let mut suppressed: u64 = 0;

            loop {
                buf.clear();
                match reader.by_ref().take(MAX_STDERR_LINE).read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(e) => {
                        log::debug!("plugin '{label}' stderr closed: {e}");
                        break;
                    }
                }
                let due = last_log.is_none_or(|t| t.elapsed() >= LOG_INTERVAL);
                if due {
                    let line = String::from_utf8_lossy(&buf);
                    let line = line.trim_end();
                    if suppressed > 0 {
                        log::debug!("plugin '{label}' stderr: {line} (+{suppressed} suppressed)");
                    } else {
                        log::debug!("plugin '{label}' stderr: {line}");
                    }
                    suppressed = 0;
                    last_log = Some(Instant::now());
                } else {
                    suppressed += 1;
                }
            }
        });
}

/// Windows process containment.
///
/// A Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` guarantees that when
/// our handle closes — including on an abnormal exit of dofek itself — every
/// process still in the job is terminated. That is the property `CLAUDE.md`
/// has been claiming; the previous code set only `CREATE_NO_WINDOW`.
///
/// **Known residual:** the child is assigned to the job immediately *after*
/// `CreateProcess` returns, not atomically at creation. A plugin that forks in
/// the microseconds before assignment leaves that fork outside the job. Closing
/// the window needs either `CREATE_SUSPENDED` plus a `ResumeThread` (which
/// `std::process::Command` gives us no thread handle for) or
/// `PROC_THREAD_ATTRIBUTE_JOB_LIST` (which needs a hand-rolled `CreateProcessW`
/// including manual pipe setup). Both are meaningful rewrites; this covers the
/// case that actually happens — a plugin that spawns helpers during normal
/// operation.
#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    /// Ceiling on processes inside one plugin's job: the plugin plus a
    /// reasonable number of helpers. A fork bomb hits this instead of the
    /// machine.
    const MAX_PROCESSES: u32 = 16;
    /// Per-process address-space ceiling inside the job.
    const MAX_PROCESS_MEMORY: usize = 1024 * 1024 * 1024;

    /// Owns a job handle; closing it terminates everything still inside.
    pub struct Job(Option<HANDLE>);

    impl Job {
        /// Create a job and put `child` in it. Every failure degrades to "no
        /// containment" with a warning rather than failing the spawn — a
        /// plugin that runs uncontained is still better than a monitor that
        /// refuses to load it.
        pub fn containing(child: &Child) -> Self {
            let job = match unsafe { CreateJobObjectW(None, None) } {
                Ok(h) => h,
                Err(e) => {
                    log::warn!("CreateJobObject failed ({e}); plugin runs uncontained");
                    return Self(None);
                }
            };

            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            info.BasicLimitInformation.ActiveProcessLimit = MAX_PROCESSES;
            info.ProcessMemoryLimit = MAX_PROCESS_MEMORY;

            let ok = unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if let Err(e) = ok {
                log::warn!("SetInformationJobObject failed ({e}); plugin runs uncontained");
                unsafe { let _ = CloseHandle(job); }
                return Self(None);
            }

            let handle = HANDLE(child.as_raw_handle());
            if let Err(e) = unsafe { AssignProcessToJobObject(job, handle) } {
                log::warn!("AssignProcessToJobObject failed ({e}); plugin runs uncontained");
                unsafe { let _ = CloseHandle(job); }
                return Self(None);
            }
            Self(Some(job))
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            if let Some(h) = self.0.take() {
                // KILL_ON_JOB_CLOSE: this is the termination.
                unsafe { let _ = CloseHandle(h); }
            }
        }
    }

    // The handle is only ever created, assigned and closed; none of that is
    // thread-affine, and `Job` is moved into `PluginProcess` which lives on
    // the supervisor thread.
    unsafe impl Send for Job {}
}
