# Security Policy

## Reporting a vulnerability

If you discover a security vulnerability in dofek, please report it privately via GitHub's [private security advisory](https://github.com/AsafSaar/dofek/security/advisories/new).

Please do **not** open a public issue, PR, or Discussion for security reports.

### What to include

- Description of the issue and its impact
- Steps to reproduce (proof of concept if possible)
- dofek version and Windows build
- Any relevant logs, screenshots, or crash dumps

### What to expect

- Acknowledgment within **5 business days**
- Initial triage and severity assessment within **10 business days**
- Regular updates as the investigation progresses
- Coordinated disclosure once a fix is available, with credit to the reporter if desired

## Scope

dofek is a local system monitor with read access to processes, GPUs, network counters, and other system state. Security-relevant surface area includes:

- Process inspection (metadata, command lines, handles)
- GPU queries via vendor APIs (NVIDIA via NVML, optional LibreHardwareMonitor HTTP fallback)
- Plugin execution (arbitrary user-provided executables)
- Configuration file parsing (TOML at startup)
- IPC between TUI and GUI processes

### In scope

- Privilege escalation from a normal user account to admin via dofek
- Arbitrary code execution via crafted config or plugin data
- Information disclosure beyond what a user-level process can already read (e.g. leaking another user's credentials or process memory)
- Denial of service against dofek itself via malformed input
- Supply-chain issues in dofek's direct dependencies

### Out of scope

- Issues requiring local admin access on the target machine. dofek runs at user level by default.
- Vulnerabilities in third-party plugins. See [Plugin security](#plugin-security) below.
- Missing code signing on binaries. Tracked separately; not a vulnerability.
- Bugs in Windows, WebView2, GPU drivers, or other external components. Report those upstream.
- Social engineering or physical access attacks.

## Plugin security

**Plugins are arbitrary executables. dofek does not sandbox them.** A malicious plugin runs with the same privileges as the user running dofek.

Treat every plugin the way you treat any other program you install: review the source, trust the author, or don't run it.

dofek's own responsibility regarding plugins is limited to:

- Parsing plugin JSON output safely (no `eval`, no injection into dofek's UI)
- Enforcing output size and rate limits to prevent DoS of the host
- Not leaking dofek internal state to plugins beyond what the documented schema specifies

If you find a way to break any of the guarantees above (for example, a malformed plugin response crashing dofek in an exploitable way, or dofek passing unexpected data to a plugin's stdin), that **is** in scope.

## Supported versions

During the 1.x phase, only the latest released minor version receives security fixes. This will be revisited at 2.0.

| Version | Supported          |
| ------- | ------------------ |
| 1.x     | ✅ Latest minor    |
| < 1.0   | ❌ Not supported   |

## Hall of fame

Reporters who responsibly disclose valid issues will be credited here (with permission).
