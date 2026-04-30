//! Plugin store — installs, lists, removes plugins on the user's behalf so
//! they never have to copy binaries onto `PATH` or hand-edit `dofek.toml`.
//!
//! Layout:
//!
//! ```text
//! <config_dir>/dofek/
//!   dofek.toml          # user-owned, never touched by this module
//!   plugins.toml        # managed file: [[plugins]] entries we installed
//!   plugins/
//!     dofek-ollama      # binaries copied here by `add()`
//!     dofek-docker
//! ```
//!
//! `Config::load` merges `[[plugins]]` from both files so the user can still
//! hand-roll a plugin in `dofek.toml` and we won't clobber it.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::config::PluginConfig;

/// Directory + path layout for the managed plugin store.
pub struct PluginStore {
    config_dir: PathBuf,
    plugins_dir: PathBuf,
    plugins_toml: PathBuf,
}

/// One installed plugin as the store sees it (binary on disk + entry in
/// `plugins.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub name: String,
    pub binary_path: PathBuf,
    pub description: String,
    pub version: String,
    pub author: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

#[derive(Deserialize, Default)]
struct PluginsTomlFile {
    #[serde(default, rename = "plugins")]
    plugins: Vec<ManagedPluginEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ManagedPluginEntry {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
    #[serde(default)]
    description: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
}

fn default_true() -> bool {
    true
}
fn default_timeout_ms() -> u64 {
    2000
}

impl PluginStore {
    /// Resolve the canonical store paths under the user's config dir. Creates
    /// the `plugins/` directory if it doesn't exist (cheap and idempotent —
    /// plugin install is the only caller and we'd create it then anyway).
    pub fn open() -> Result<Self> {
        let base = dirs::config_dir().context("could not resolve user config dir")?;
        let config_dir = base.join("dofek");
        let plugins_dir = config_dir.join("plugins");
        let plugins_toml = config_dir.join("plugins.toml");
        fs::create_dir_all(&plugins_dir)
            .with_context(|| format!("failed to create {}", plugins_dir.display()))?;
        Ok(Self {
            config_dir,
            plugins_dir,
            plugins_toml,
        })
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }
    pub fn plugins_toml(&self) -> &Path {
        &self.plugins_toml
    }

    /// Returns the managed `[[plugins]]` entries as `PluginConfig`s, ready to
    /// be merged into the in-memory `Config`.
    pub fn load_plugin_configs(&self) -> Vec<PluginConfig> {
        let entries = match self.read_managed() {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Failed to read {}: {e}", self.plugins_toml.display());
                return Vec::new();
            }
        };
        entries
            .into_iter()
            .map(|e| PluginConfig {
                name: e.name,
                command: e.command,
                args: e.args,
                enabled: e.enabled,
                timeout_ms: e.timeout_ms,
            })
            .collect()
    }

    pub fn list(&self) -> Result<Vec<InstalledPlugin>> {
        let entries = self.read_managed()?;
        Ok(entries
            .into_iter()
            .map(|e| InstalledPlugin {
                binary_path: self.plugins_dir.join(&e.command),
                name: e.name,
                description: e.description,
                version: e.version,
                author: e.author,
                args: e.args,
                enabled: e.enabled,
            })
            .collect())
    }

    /// Copy `source` into the managed `plugins/` directory, probe its
    /// manifest, and append a `[[plugins]]` entry to `plugins.toml`.
    ///
    /// On macOS, clears the `com.apple.quarantine` xattr so Gatekeeper doesn't
    /// silently kill the binary on first launch (same fix users do manually
    /// today). On Unix, marks the binary as executable.
    pub fn add(&self, source: &Path, args: Vec<String>) -> Result<InstalledPlugin> {
        if !source.is_file() {
            bail!("not a file: {}", source.display());
        }

        // Use the source filename verbatim — plugin authors picked the name
        // (e.g. `dofek-ollama`), and matching it makes the eventual
        // [[plugins]] command field obvious.
        let file_name = source
            .file_name()
            .ok_or_else(|| anyhow!("source has no file name: {}", source.display()))?
            .to_owned();

        let dest = self.plugins_dir.join(&file_name);
        fs::copy(source, &dest)
            .with_context(|| format!("failed to copy {} → {}", source.display(), dest.display()))?;
        make_executable(&dest)?;
        clear_quarantine(&dest);

        // Probe the binary so we can use the plugin's own manifest as the
        // canonical name/description/version, falling back to the filename if
        // the binary doesn't speak the protocol.
        let manifest = probe_manifest(&dest, &args).unwrap_or_default();
        let display_name = if manifest.name.is_empty() {
            file_name.to_string_lossy().to_string()
        } else {
            manifest.name.clone()
        };

        // Reject duplicates by managed-name. If the user is reinstalling, they
        // should `remove` first — this prevents accidental dupes in
        // plugins.toml.
        let mut existing = self.read_managed().unwrap_or_default();
        if existing.iter().any(|e| e.name == display_name) {
            // Best-effort cleanup of the file we just wrote.
            let _ = fs::remove_file(&dest);
            bail!(
                "plugin '{display_name}' is already installed — run `dofek-tui plugins remove {display_name}` first"
            );
        }

        let entry = ManagedPluginEntry {
            name: display_name.clone(),
            command: file_name.to_string_lossy().to_string(),
            args,
            enabled: true,
            timeout_ms: 2000,
            description: manifest.description,
            version: manifest.version,
            author: manifest.author,
        };
        existing.push(entry.clone());
        self.write_managed(&existing)?;

        Ok(InstalledPlugin {
            name: entry.name,
            binary_path: dest,
            description: entry.description,
            version: entry.version,
            author: entry.author,
            args: entry.args,
            enabled: entry.enabled,
        })
    }

    /// Remove the `[[plugins]]` entry and delete the binary from the managed
    /// directory.
    pub fn remove(&self, name: &str) -> Result<()> {
        let mut entries = self.read_managed()?;
        let idx = entries
            .iter()
            .position(|e| e.name == name)
            .ok_or_else(|| anyhow!("no managed plugin named '{name}'"))?;
        let removed = entries.remove(idx);
        self.write_managed(&entries)?;

        let bin = self.plugins_dir.join(&removed.command);
        if bin.exists()
            && let Err(e) = fs::remove_file(&bin)
        {
            log::warn!("removed entry but failed to delete {}: {e}", bin.display());
        }
        Ok(())
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let mut entries = self.read_managed()?;
        let entry = entries
            .iter_mut()
            .find(|e| e.name == name)
            .ok_or_else(|| anyhow!("no managed plugin named '{name}'"))?;
        entry.enabled = enabled;
        self.write_managed(&entries)
    }

    fn read_managed(&self) -> Result<Vec<ManagedPluginEntry>> {
        if !self.plugins_toml.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.plugins_toml)
            .with_context(|| format!("read {}", self.plugins_toml.display()))?;
        let parsed: PluginsTomlFile = toml::from_str(&raw)
            .with_context(|| format!("parse {}", self.plugins_toml.display()))?;
        Ok(parsed.plugins)
    }

    fn write_managed(&self, entries: &[ManagedPluginEntry]) -> Result<()> {
        let mut out = String::new();
        out.push_str("# Managed by `dofek-tui plugins ...` — do not edit by hand.\n");
        out.push_str("# To add or remove plugins, use the dofek CLI or GUI.\n\n");
        for e in entries {
            #[derive(Serialize)]
            struct PluginsWrap<'a> {
                plugins: [&'a ManagedPluginEntry; 1],
            }
            let wrap = PluginsWrap { plugins: [e] };
            let chunk = toml::to_string(&wrap).context("serialize plugins.toml")?;
            out.push_str(&chunk);
            out.push('\n');
        }
        fs::write(&self.plugins_toml, out)
            .with_context(|| format!("write {}", self.plugins_toml.display()))?;
        Ok(())
    }
}

/// The subset of [`dofek_plugin_protocol::PluginManifest`] we surface to users.
#[derive(Debug, Default, Clone)]
pub struct ProbedManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

/// Spawn the candidate binary, send a single `poll` request, parse the
/// response, and return its manifest. Best-effort — if the binary isn't a
/// dofek plugin we fall back to filename-based defaults at the call site.
fn probe_manifest(binary: &Path, args: &[String]) -> Result<ProbedManifest> {
    let mut cmd = Command::new(binary);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawn {}", binary.display()))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(
            stdin,
            r#"{{"type":"poll","timestamp_ms":0,"processes":[]}}"#
        );
        let _ = stdin.flush();
        // dropping stdin closes it — ensures the plugin's read loop sees EOF
        // after the single message and we can wait() cleanly even if it
        // doesn't honor shutdown.
        drop(stdin);
    }

    let stdout = child.stdout.take().context("plugin stdout missing")?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // The deadline guards against a plugin that opens stdout but never writes;
    // read_line is blocking, so the bound is wall-clock-only — if the plugin
    // hangs forever we would too. A future refactor could set a non-blocking
    // read; for now we accept "single attempt with an upper bound" semantics
    // and silence clippy::never_loop because every arm exits the loop.
    let deadline = Instant::now() + Duration::from_millis(2500);
    #[allow(clippy::never_loop)]
    let response = loop {
        if Instant::now() > deadline {
            let _ = child.kill();
            bail!("plugin probe timed out (no manifest within 2.5s)");
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => bail!("plugin closed stdout without responding"),
            Ok(_) => break line.trim().to_string(),
            Err(e) => bail!("plugin probe read error: {e}"),
        }
    };

    // Send shutdown so the binary exits cleanly. Best-effort.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin, r#"{{"type":"shutdown"}}"#);
    }
    let _ = child.wait();

    let parsed: dofek_plugin_protocol::PollResponse =
        serde_json::from_str(&response).with_context(|| format!("parse probe response: {response}"))?;
    let m = parsed.manifest.unwrap_or_else(|| {
        dofek_plugin_protocol::PluginManifest {
            name: String::new(),
            version: String::new(),
            description: String::new(),
            author: String::new(),
        }
    });
    Ok(ProbedManifest {
        name: m.name,
        version: m.version,
        description: m.description,
        author: m.author,
    })
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// macOS quarantines binaries copied from external sources (downloads, USB,
/// etc.). The user-facing fix is a manual `xattr -dr com.apple.quarantine`.
/// Since the user explicitly asked us to install this binary, we run the
/// equivalent automatically — same trust boundary as them double-clicking it.
#[cfg(target_os = "macos")]
fn clear_quarantine(path: &Path) {
    let _ = Command::new("xattr")
        .args(["-d", "com.apple.quarantine"])
        .arg(path)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status();
}

#[cfg(not(target_os = "macos"))]
fn clear_quarantine(_path: &Path) {}

/// Resolve a plugin `command` string to an absolute path. Tries:
/// 1. The string as-is (absolute paths or `PATH`-resolved names).
/// 2. `<config_dir>/dofek/plugins/<command>` — managed install location.
///
/// Returns `command` unchanged if neither exists; spawn will then surface a
/// clear "not found" error.
pub fn resolve_command(command: &str) -> String {
    let direct = Path::new(command);
    if direct.is_absolute() && direct.exists() {
        return command.to_string();
    }
    if let Some(dir) = dirs::config_dir() {
        let candidate = dir.join("dofek").join("plugins").join(command);
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }
    command.to_string()
}
