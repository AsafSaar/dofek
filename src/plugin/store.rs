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
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::process::{PluginProcess, ReadEvent};
use super::sanitize;
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

/// Upper bound on an installed plugin binary. Generous for a real plugin
/// (the first-party ones are single-digit MB) and small enough that a
/// mis-picked file can't fill the config volume.
const MAX_PLUGIN_BYTES: u64 = 256 * 1024 * 1024;

/// Reduce a source file name to something safe to use as both a path
/// component and a `command` value in plugins.toml.
///
/// Rejects rather than rewrites: silently renaming the user's binary would be
/// confusing, and every legitimate plugin name already fits.
fn sanitize_file_name(name: &str) -> Result<String> {
    if name.is_empty() || name == "." || name == ".." {
        bail!("invalid plugin file name: {name:?}");
    }
    if name.len() > 128 {
        bail!("plugin file name is too long ({} chars, max 128)", name.len());
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
    {
        bail!(
            "plugin file name {name:?} contains {bad:?}; only ASCII letters, \
             digits, '.', '_' and '-' are allowed"
        );
    }
    // A leading dot would create a hidden file and, with `..`-like names,
    // invites confusion about what is actually installed.
    if name.starts_with('.') {
        bail!("plugin file name must not start with '.': {name:?}");
    }
    Ok(name.to_string())
}

/// Bound the argument vector stored in plugins.toml and passed on every spawn.
fn validate_plugin_args(args: &[String]) -> Result<()> {
    const MAX_ARGS: usize = 32;
    const MAX_ARG_BYTES: usize = 1024;

    if args.len() > MAX_ARGS {
        bail!("too many plugin arguments ({}, max {MAX_ARGS})", args.len());
    }
    for a in args {
        if a.len() > MAX_ARG_BYTES {
            bail!("plugin argument is {} bytes, max {MAX_ARG_BYTES}", a.len());
        }
        if a.chars().any(|c| c == '\0' || c.is_control()) {
            bail!("plugin argument contains a control character: {a:?}");
        }
    }
    Ok(())
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
        validate_plugin_args(&args)?;

        // Open once and validate the *handle*, not the path. Checking
        // `source.is_file()` and then copying by path leaves a window in which
        // the path can be swapped for something else between the two.
        let mut src = fs::File::open(source)
            .with_context(|| format!("cannot open {}", source.display()))?;
        let meta = src
            .metadata()
            .with_context(|| format!("cannot stat {}", source.display()))?;
        if !meta.is_file() {
            bail!("not a regular file: {}", source.display());
        }
        if meta.len() > MAX_PLUGIN_BYTES {
            bail!(
                "plugin binary is {} bytes, over the {MAX_PLUGIN_BYTES}-byte limit",
                meta.len()
            );
        }

        // The file name becomes both a path component and the `command` field
        // in plugins.toml, so it has to be a plain name.
        let raw_name = source
            .file_name()
            .ok_or_else(|| anyhow!("source has no file name: {}", source.display()))?
            .to_string_lossy()
            .to_string();
        let file_name = sanitize_file_name(&raw_name)?;

        let dest = self.plugins_dir.join(&file_name);
        // `create_new` fails if anything already exists at `dest`, including a
        // symlink — so a pre-planted link can't redirect this write elsewhere.
        // It also makes the "already installed" case fail before we copy.
        let mut dst = match fs::OpenOptions::new().write(true).create_new(true).open(&dest) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => bail!(
                "a plugin binary named '{file_name}' is already installed — run \
                 `dofek-tui plugins remove` first"
            ),
            Err(e) => {
                return Err(e).with_context(|| format!("failed to create {}", dest.display()));
            }
        };
        if let Err(e) = std::io::copy(&mut src, &mut dst) {
            drop(dst);
            let _ = fs::remove_file(&dest);
            return Err(e).with_context(|| format!("failed to copy into {}", dest.display()));
        }
        drop(dst);
        make_executable(&dest)?;
        clear_quarantine(&dest);

        // Probe the binary so we can use the plugin's own manifest as the
        // canonical name/description/version, falling back to the filename if
        // the binary doesn't speak the protocol.
        let manifest = probe_manifest(&dest, &args).unwrap_or_default();
        let display_name = if manifest.name.is_empty() {
            file_name.clone()
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
            command: file_name.clone(),
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

/// How long a candidate binary gets to identify itself before we give up and
/// fall back to filename-derived metadata.
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(2500);

/// Spawn the candidate binary, send a single `poll` request, parse the
/// response, and return its manifest. Best-effort — if the binary isn't a
/// dofek plugin we fall back to filename-based defaults at the call site.
///
/// Public so the runtime tests can drive it directly: it sits on the
/// `plugins_add` IPC path, where a binary that opens stdout and never writes
/// used to park a Tauri worker permanently, and that deserves its own
/// regression test rather than only being reachable through a real install.
///
/// Built on the same [`PluginProcess`] primitive as the runtime, so the
/// deadline here is real. The previous version did a blocking `read_line` and
/// only consulted its deadline *before* the read — meaning a binary that
/// opened stdout and never wrote would hang it forever. That mattered more
/// here than in the collector: this runs on a Tauri IPC worker, so a single
/// bad pick could park one permanently.
pub fn probe_manifest(binary: &Path, args: &[String]) -> Result<ProbedManifest> {
    let mut proc = PluginProcess::spawn_resolved(binary, args, "probe")
        .with_context(|| format!("spawn {}", binary.display()))?;

    let request = serde_json::to_string(&dofek_plugin_protocol::PollRequest::with_seq(0, 1, Vec::new()))
        .context("serialize probe request")?;
    proc.send_line(&request)
        .context("plugin closed stdin before the probe request")?;

    let response = match proc.recv_timeout(PROBE_TIMEOUT) {
        Ok(ReadEvent::Line(l)) => l,
        Ok(ReadEvent::Closed(why)) => bail!("plugin probe failed: {why}"),
        Err(_) => bail!("plugin probe timed out (no manifest within {PROBE_TIMEOUT:?})"),
    };

    // Ask it to exit; `PluginProcess` escalates to killing the process group
    // if it doesn't, and its `Drop` covers every early return above.
    proc.shutdown_and_kill();

    let parsed: dofek_plugin_protocol::PollResponse = serde_json::from_str(&response)
        .with_context(|| format!("parse probe response: {response}"))?;
    let m = parsed.manifest.unwrap_or_else(|| dofek_plugin_protocol::PluginManifest {
        name: String::new(),
        version: String::new(),
        description: String::new(),
        author: String::new(),
    });
    // The probe result is written into plugins.toml and shown in both UIs, so
    // it goes through the same bounds as a runtime response.
    Ok(ProbedManifest {
        name: sanitize::clean_short(&m.name),
        version: sanitize::clean_short(&m.version),
        description: sanitize::clean_long(&m.description),
        author: sanitize::clean_short(&m.author),
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
    use std::process::{Command, Stdio};
    let _ = Command::new("/usr/bin/xattr")
        .args(["-d", "com.apple.quarantine"])
        .arg(path)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status();
}

#[cfg(not(target_os = "macos"))]
fn clear_quarantine(_path: &Path) {}

/// Resolve a plugin `command` string to a concrete executable path.
///
/// Only two shapes are accepted:
/// 1. An absolute path to an existing regular file.
/// 2. A bare file name that exists inside the managed plugins directory.
///
/// Everything else is an error. In particular this **never** consults `PATH`
/// and never resolves relative to the current directory.
///
/// That matters because the previous version returned the string unchanged
/// when it couldn't resolve it, handing the decision to `Command::new` — which
/// searches `PATH`, and on Windows searches the current working directory
/// *before* `PATH`. Combined with a config file that can declare
/// `[[plugins]]`, an unresolvable bare name meant "run whatever happens to sit
/// next to the config".
pub fn resolve_command(command: &str) -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("could not resolve user config dir")?
        .join("dofek")
        .join("plugins");
    resolve_command_among(&dir, &bundled_dirs(), command)
}

/// The first-party plugins the installers ship alongside dofek itself.
///
/// This list is deliberately closed. Probing the executable's directory for
/// *any* bare name would quietly re-introduce what PR 3 removed: on a `.deb`
/// install `current_exe()` sits in `/usr/bin`, so "look next to the binary"
/// would be a slice of `PATH` by another name. Restricting the probe to names
/// we actually bundle means the lookup can never be turned into a general
/// executable search, however a `dofek.toml` is written.
pub const BUNDLED_PLUGINS: &[&str] = &["dofek-ollama", "dofek-docker", "dofek-net-ping"];

/// Directories the installers may have placed a bundled plugin in.
///
/// Tauri's `externalBin` puts sidecars next to the main executable, which
/// covers the MSI and the `.app`. AppImage mounts its payload read-only and
/// exports `$APPDIR`, where the binaries land in `usr/bin`.
fn bundled_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        dirs.push(parent.to_path_buf());
    }
    if let Some(appdir) = std::env::var_os("APPDIR") {
        dirs.push(PathBuf::from(appdir).join("usr").join("bin"));
    }
    dirs
}

/// True if `command` names one of the [`BUNDLED_PLUGINS`]. Tolerates the
/// `.exe` suffix so a Windows `dofek.toml` written either way works.
fn is_bundled_plugin(command: &str) -> bool {
    let stem = command.strip_suffix(".exe").unwrap_or(command);
    BUNDLED_PLUGINS.contains(&stem)
}

/// The target triple this binary was built for, emitted by `build.rs` from
/// Cargo's `TARGET`. `std` exposes no equivalent.
const TARGET_TRIPLE: &str = env!("DOFEK_TARGET_TRIPLE");

/// File names a bundled plugin might have on disk, in preference order.
///
/// Tauri stages an `externalBin` as `<name>-<triple>` and the published docs do
/// not say whether the suffix survives into the installed bundle. Rather than
/// encode a guess about a dependency's internals, try both: the plain name
/// first, then the suffixed one. A miss costs one failed `canonicalize`.
fn bundled_file_names(command: &str) -> Vec<String> {
    let (stem, ext) = match command.strip_suffix(".exe") {
        Some(stem) => (stem, ".exe"),
        None => (command, ""),
    };
    let mut names = vec![command.to_string()];
    if !TARGET_TRIPLE.is_empty() {
        names.push(format!("{stem}-{TARGET_TRIPLE}{ext}"));
    }
    names
}

/// [`resolve_command`] against an explicit managed-plugins directory, so tests
/// can point it at a tempdir instead of the real user config directory.
///
/// Bundled-plugin resolution is not exercised by this entry point — see
/// [`resolve_command_among`].
pub fn resolve_command_in(plugins_dir: &Path, command: &str) -> Result<PathBuf> {
    resolve_command_among(plugins_dir, &[], command)
}

/// [`resolve_command_in`] with the bundled-plugin search path injected, so the
/// probe is testable without reaching for `current_exe()`.
///
/// Resolution order is managed-dir first: a plugin the user explicitly
/// installed wins over the copy that shipped with the app, because the
/// installed one may well be newer.
pub fn resolve_command_among(
    plugins_dir: &Path,
    bundled: &[PathBuf],
    command: &str,
) -> Result<PathBuf> {
    if command.is_empty() {
        bail!("plugin command is empty");
    }
    let given = Path::new(command);

    if given.is_absolute() {
        // Canonicalize so the "is it a regular file" check and the path we
        // hand to Command::new refer to the same resolved target.
        let real = given
            .canonicalize()
            .with_context(|| format!("plugin command not found: {command}"))?;
        if !real.is_file() {
            bail!("plugin command is not a regular file: {command}");
        }
        return Ok(real);
    }

    // Relative paths are rejected outright rather than joined onto the
    // plugins dir: `../../../bin/sh` would otherwise escape it, and a relative
    // command in a config file is exactly the CWD-dependent behaviour this
    // function exists to prevent.
    if command.contains('/') || command.contains('\\') || given.components().count() != 1 {
        bail!(
            "plugin command must be an absolute path or a bare file name, got: {command}"
        );
    }

    if let Some(found) = resolve_inside(plugins_dir, command)? {
        return Ok(found);
    }

    // Fall back to the copy the installer bundled, for first-party plugins
    // only. This is what makes a bundled install one-click and offline: the
    // plugin is already on disk, so `plugins.toml` can name it without the
    // user hunting down a binary.
    if is_bundled_plugin(command) {
        for dir in bundled {
            for name in bundled_file_names(command) {
                if let Some(found) = resolve_inside(dir, &name)? {
                    return Ok(found);
                }
            }
        }
    }

    bail!("plugin '{command}' is not installed in {}", plugins_dir.display())
}

/// Resolve a bare `name` strictly inside `dir`, or `Ok(None)` if it isn't
/// there. `Err` is reserved for a name that *is* there but is unusable — a
/// directory, or a symlink escaping `dir` — so a planted link is reported
/// rather than silently skipped in favour of the next candidate directory.
fn resolve_inside(dir: &Path, name: &str) -> Result<Option<PathBuf>> {
    let Ok(real) = dir.join(name).canonicalize() else {
        return Ok(None); // not present here
    };
    if !real.is_file() {
        bail!("plugin '{name}' is not a regular file");
    }
    // canonicalize follows symlinks, so a link planted in `dir` could point
    // anywhere. Require the resolved target to still be inside `dir`.
    let real_dir = dir
        .canonicalize()
        .with_context(|| format!("plugin directory unreadable: {}", dir.display()))?;
    if !real.starts_with(&real_dir) {
        bail!("plugin '{name}' resolves outside {}", real_dir.display());
    }
    Ok(Some(real))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, b"#!/bin/sh\n").unwrap();
        p
    }

    /// A managed plugin — a bare name inside the plugins dir — resolves to its
    /// canonical absolute path.
    #[test]
    fn bare_name_resolves_into_the_plugins_dir() {
        let dir = tempfile::tempdir().unwrap();
        let installed = touch(dir.path(), "dofek-ollama");

        let got = resolve_command_in(dir.path(), "dofek-ollama").unwrap();
        assert_eq!(got, installed.canonicalize().unwrap());
    }

    #[test]
    fn existing_absolute_path_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let exe = touch(dir.path(), "elsewhere");
        let plugins = tempfile::tempdir().unwrap();

        let got = resolve_command_in(plugins.path(), &exe.to_string_lossy()).unwrap();
        assert_eq!(got, exe.canonicalize().unwrap());
    }

    /// The core of the fix: an unresolvable command is an error, not a string
    /// handed to `Command::new` to look up on PATH (or, on Windows, in the
    /// current directory).
    #[test]
    fn unknown_bare_name_is_rejected_rather_than_passed_to_path() {
        let plugins = tempfile::tempdir().unwrap();
        for name in ["python3", "sh", "cmd.exe", "definitely-not-installed"] {
            let err = resolve_command_in(plugins.path(), name).unwrap_err();
            assert!(
                format!("{err:#}").contains("not installed"),
                "{name}: unexpected error {err:#}"
            );
        }
    }

    /// Relative paths are rejected outright — this is the shape that made a
    /// hostile `dofek.toml` in the working directory dangerous.
    #[test]
    fn relative_paths_are_rejected() {
        let plugins = tempfile::tempdir().unwrap();
        // Even when the file genuinely exists inside the plugins dir, a
        // relative form is refused: accepting it would mean resolving against
        // the process's current directory.
        touch(plugins.path(), "real-plugin");

        for cmd in [
            "./pwned.sh",
            "../pwned.sh",
            "../../../bin/sh",
            "sub/dir/plugin",
            "./real-plugin",
            r"..\windows\system32\cmd.exe",
        ] {
            let err = resolve_command_in(plugins.path(), cmd).unwrap_err();
            let msg = format!("{err:#}");
            assert!(
                msg.contains("absolute path or a bare file name") || msg.contains("not installed"),
                "{cmd}: unexpected error {msg}"
            );
        }
    }

    #[test]
    fn empty_command_is_rejected() {
        let plugins = tempfile::tempdir().unwrap();
        // Previously this resolved to the plugins *directory*, because
        // `dir.join("")` is `dir` and the check was `exists()`.
        let err = resolve_command_in(plugins.path(), "").unwrap_err();
        assert!(format!("{err:#}").contains("empty"), "{err:#}");
    }

    #[test]
    fn a_directory_is_not_a_command() {
        let plugins = tempfile::tempdir().unwrap();
        fs::create_dir(plugins.path().join("somedir")).unwrap();
        let err = resolve_command_in(plugins.path(), "somedir").unwrap_err();
        assert!(format!("{err:#}").contains("not a regular file"), "{err:#}");
    }

    #[test]
    fn missing_absolute_path_is_rejected() {
        let plugins = tempfile::tempdir().unwrap();
        let absent = if cfg!(windows) { r"C:\nope\absent.exe" } else { "/nope/absent" };
        let err = resolve_command_in(plugins.path(), absent).unwrap_err();
        assert!(format!("{err:#}").contains("not found"), "{err:#}");
    }

    /// canonicalize() follows symlinks, so a link planted in the managed
    /// directory could otherwise point at any binary on the system and still
    /// look like a legitimately installed plugin.
    #[cfg(unix)]
    #[test]
    fn symlink_out_of_the_plugins_dir_is_rejected() {
        let outside = tempfile::tempdir().unwrap();
        let target = touch(outside.path(), "evil");
        let plugins = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(&target, plugins.path().join("looks-legit")).unwrap();

        let err = resolve_command_in(plugins.path(), "looks-legit").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("resolves outside"), "{msg}");
        // The message names the directory the link escaped, which is the thing
        // a user needs in order to go look at it.
        assert!(
            msg.contains(&*plugins.path().canonicalize().unwrap().to_string_lossy()),
            "{msg}"
        );
    }

    /// v1.7 bundles the three first-party plugins with the installers, so a
    /// bare name that isn't in the managed directory falls back to the
    /// directory holding the running executable.
    #[test]
    fn bundled_first_party_plugin_resolves_next_to_the_executable() {
        let plugins = tempfile::tempdir().unwrap();
        let bundled = tempfile::tempdir().unwrap();
        let shipped = touch(bundled.path(), "dofek-ollama");

        let got = resolve_command_among(
            plugins.path(),
            &[bundled.path().to_path_buf()],
            "dofek-ollama",
        )
        .unwrap();
        assert_eq!(got, shipped.canonicalize().unwrap());
    }

    /// A plugin the user explicitly installed beats the bundled copy — theirs
    /// may be newer, and the install was a deliberate act.
    #[test]
    fn managed_install_wins_over_the_bundled_copy() {
        let plugins = tempfile::tempdir().unwrap();
        let bundled = tempfile::tempdir().unwrap();
        let installed = touch(plugins.path(), "dofek-docker");
        touch(bundled.path(), "dofek-docker");

        let got = resolve_command_among(
            plugins.path(),
            &[bundled.path().to_path_buf()],
            "dofek-docker",
        )
        .unwrap();
        assert_eq!(got, installed.canonicalize().unwrap());
    }

    /// The whole point of keeping `BUNDLED_PLUGINS` a closed list: on a `.deb`
    /// install the executable's directory *is* `/usr/bin`, so probing it for
    /// arbitrary names would be a PATH lookup wearing a different hat — exactly
    /// what PR 3 removed.
    #[test]
    fn the_executable_dir_is_not_a_general_path_lookup() {
        let plugins = tempfile::tempdir().unwrap();
        let exe_dir = tempfile::tempdir().unwrap();
        // Stand in for the binaries that really do sit next to dofek in /usr/bin.
        for name in ["sh", "curl", "python3", "dofek-tui", "totally-unrelated"] {
            touch(exe_dir.path(), name);
        }

        for name in ["sh", "curl", "python3", "dofek-tui", "totally-unrelated"] {
            let err = resolve_command_among(
                plugins.path(),
                &[exe_dir.path().to_path_buf()],
                name,
            )
            .unwrap_err();
            assert!(
                format!("{err:#}").contains("not installed"),
                "{name} resolved from the executable dir but is not a bundled plugin"
            );
        }
    }

    /// Tauri stages an `externalBin` as `<name>-<triple>` and the published
    /// docs do not say whether that suffix survives into the installed bundle.
    /// Both spellings resolve, so the feature does not rest on a guess about a
    /// dependency's internals.
    #[test]
    fn a_triple_suffixed_sidecar_also_resolves() {
        let plugins = tempfile::tempdir().unwrap();
        let bundled = tempfile::tempdir().unwrap();
        let triple = super::TARGET_TRIPLE;
        assert!(!triple.is_empty(), "build.rs must emit DOFEK_TARGET_TRIPLE");
        let shipped = touch(bundled.path(), &format!("dofek-ollama-{triple}"));

        let got = resolve_command_among(
            plugins.path(),
            &[bundled.path().to_path_buf()],
            "dofek-ollama",
        )
        .unwrap();
        assert_eq!(got, shipped.canonicalize().unwrap());
    }

    /// The unsuffixed name wins when both are present — that is the plain
    /// install layout, and preferring it keeps behaviour stable if Tauri's
    /// naming ever changes.
    #[test]
    fn the_plain_name_is_preferred_over_the_suffixed_one() {
        let plugins = tempfile::tempdir().unwrap();
        let bundled = tempfile::tempdir().unwrap();
        let plain = touch(bundled.path(), "dofek-docker");
        touch(bundled.path(), &format!("dofek-docker-{}", super::TARGET_TRIPLE));

        let got = resolve_command_among(
            plugins.path(),
            &[bundled.path().to_path_buf()],
            "dofek-docker",
        )
        .unwrap();
        assert_eq!(got, plain.canonicalize().unwrap());
    }

    /// Windows configs may spell the command either way.
    #[test]
    fn bundled_lookup_tolerates_the_exe_suffix() {
        let plugins = tempfile::tempdir().unwrap();
        let bundled = tempfile::tempdir().unwrap();
        let shipped = touch(bundled.path(), "dofek-net-ping.exe");

        let got = resolve_command_among(
            plugins.path(),
            &[bundled.path().to_path_buf()],
            "dofek-net-ping.exe",
        )
        .unwrap();
        assert_eq!(got, shipped.canonicalize().unwrap());
    }

    /// A symlink planted in a bundled directory gets the same containment check
    /// as one in the managed directory — and is reported rather than skipped in
    /// favour of the next candidate.
    #[cfg(unix)]
    #[test]
    fn symlink_escape_from_a_bundled_dir_is_rejected() {
        let outside = tempfile::tempdir().unwrap();
        let target = touch(outside.path(), "evil");
        let plugins = tempfile::tempdir().unwrap();
        let bundled = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(&target, bundled.path().join("dofek-ollama")).unwrap();

        let err = resolve_command_among(
            plugins.path(),
            &[bundled.path().to_path_buf()],
            "dofek-ollama",
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("resolves outside"), "{err:#}");
    }

    /// A symlink that stays inside the directory is fine — that is a normal
    /// way to have two names for one plugin.
    #[cfg(unix)]
    #[test]
    fn symlink_within_the_plugins_dir_is_allowed() {
        let plugins = tempfile::tempdir().unwrap();
        let target = touch(plugins.path(), "dofek-ollama");
        std::os::unix::fs::symlink(&target, plugins.path().join("ollama")).unwrap();

        let got = resolve_command_in(plugins.path(), "ollama").unwrap();
        assert_eq!(got, target.canonicalize().unwrap());
    }

    #[test]
    fn nonexistent_plugins_dir_is_an_error_not_a_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        assert!(resolve_command_in(&missing, "anything").is_err());
    }

    // --- install-time input validation ---

    #[test]
    fn file_names_are_restricted_to_plain_ascii() {
        for ok in ["dofek-ollama", "plugin.exe", "my_plugin", "a1", "x.y.z"] {
            assert_eq!(sanitize_file_name(ok).unwrap(), ok);
        }
        for bad in [
            "",                    // empty
            ".",                   // current dir
            "..",                  // parent dir
            ".hidden",             // leading dot
            "with space",          // whitespace
            "semi;colon",          // shell metacharacter
            "quote\"q",            // quoting
            "sub/dir",             // path separator
            "back\\slash",         // Windows separator
            "unicode-字",          // non-ASCII
            "nul\0byte",           // NUL
        ] {
            assert!(sanitize_file_name(bad).is_err(), "should reject {bad:?}");
        }
        assert!(sanitize_file_name(&"a".repeat(129)).is_err(), "over length limit");
        assert!(sanitize_file_name(&"a".repeat(128)).is_ok(), "at length limit");
    }

    #[test]
    fn plugin_args_are_bounded() {
        assert!(validate_plugin_args(&[]).is_ok());
        assert!(validate_plugin_args(&["--host".into(), "http://x".into()]).is_ok());

        let too_many: Vec<String> = (0..33).map(|i| i.to_string()).collect();
        assert!(validate_plugin_args(&too_many).is_err());

        assert!(validate_plugin_args(&["a".repeat(1025)]).is_err());
        assert!(validate_plugin_args(&["a".repeat(1024)]).is_ok());

        assert!(validate_plugin_args(&["with\0nul".into()]).is_err());
        assert!(validate_plugin_args(&["with\nnewline".into()]).is_err());
        assert!(validate_plugin_args(&["with\rcarriage".into()]).is_err());
    }
}
