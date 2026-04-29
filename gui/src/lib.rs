use std::sync::{Arc, Mutex};
use std::time::Instant;

use dofek::config::Config;
use dofek::data::DataSnapshot;
use dofek::settings::UserSettings;
use dofek::telemetry::{self, TelemetryEvent, TelemetryHandle};
use dofek::GpuEmptyState;

mod tray;

/// Shared state: the latest data snapshot from the collector thread.
pub struct AppState {
    pub snapshot: Arc<Mutex<DataSnapshot>>,
    pub config: Config,
    pub settings: Arc<Mutex<UserSettings>>,
    pub telemetry: TelemetryHandle,
}

/// Tauri command: returns the latest system data snapshot as JSON.
#[tauri::command]
fn get_snapshot(state: tauri::State<'_, AppState>) -> DataSnapshot {
    state.snapshot.lock().unwrap().clone()
}

/// Tauri command: returns GPU device definitions (name + VRAM total) for the frontend.
#[tauri::command]
fn get_gpu_info(state: tauri::State<'_, AppState>) -> Vec<GpuDef> {
    let snap = state.snapshot.lock().unwrap();
    snap.gpus.iter().map(|g| GpuDef {
        name: g.name.clone(),
        vram_total_mb: g.vram_total_mb,
    }).collect()
}

#[derive(serde::Serialize)]
pub struct GpuDef {
    pub name: String,
    pub vram_total_mb: f32,
}

/// Tauri command: returns platform-appropriate labels for the no-GPU empty state.
/// Apple Silicon Macs aren't "no GPU" — they have an integrated GPU sharing
/// system memory, so we surface the chip name instead.
#[tauri::command]
fn get_platform_info() -> GpuEmptyState {
    dofek::gpu_empty_state().clone()
}

/// Tauri command: returns the GUI's compile-time package version so the
/// frontend doesn't have to hardcode it (and drift away from Cargo.toml).
#[tauri::command]
fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Tauri command: queries GitHub for the latest Dofek release and returns
/// version comparison + release URL. Notify-only — never downloads anything.
/// The frontend decides how loud to be about the result.
#[tauri::command]
async fn check_for_update() -> Result<dofek::update::UpdateInfo, String> {
    // Off-thread because ureq blocks; the async signature lets Tauri schedule
    // it without parking the IPC worker.
    tauri::async_runtime::spawn_blocking(|| dofek::update::check().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// Tauri command: open an arbitrary https URL in the user's default browser.
/// Used by the "update available" affordance in the topbar / settings.
#[tauri::command]
fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    if !url.starts_with("https://") {
        return Err("only https URLs are allowed".into());
    }
    #[allow(deprecated)]
    app.shell().open(url, None).map_err(|e| e.to_string())
}

/// Tauri command: opens the bundled offline manual.html in the user's default browser.
#[tauri::command]
fn open_manual(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    use tauri_plugin_shell::ShellExt;
    let manual = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("manual.html");
    #[allow(deprecated)]
    app.shell()
        .open(manual.to_string_lossy().to_string(), None)
        .map_err(|e| e.to_string())
}

/// Tauri command: returns the current user settings.
#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> UserSettings {
    state.settings.lock().unwrap().clone()
}

/// Tauri command: saves UI settings to disk, preserving telemetry/identity fields.
#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    settings: UserSettings,
) -> Result<(), String> {
    // Update the in-memory state first, then release the settings lock before
    // touching anything else. The relay thread acquires snapshot-then-settings
    // each tick, so we must not hold the settings lock while reaching for the
    // snapshot lock here — that would invert the order and risk a deadlock.
    let merged = {
        let mut current = state.settings.lock().unwrap();
        // Preserve telemetry and identity fields — only set_telemetry_choice should change these
        let m = UserSettings {
            anonymous_id: current.anonymous_id.clone(),
            telemetry_prompted: current.telemetry_prompted,
            telemetry_enabled: current.telemetry_enabled,
            ..settings
        };
        *current = m.clone();
        m
    };

    // Apply tray-affecting settings synchronously so the user sees the change
    // land immediately. Without this, the tray would only pick up new
    // `tray_display_mode` / `tray_show_text` values on the next data-collector
    // tick (≥1 s) — slow enough that users perceived it as "needs a restart".
    if merged.enable_tray {
        let snap = state.snapshot.lock().unwrap().clone();
        tray::update(&app, &snap, &merged);
    }

    merged.save().map_err(|e| e.to_string())
}

/// Tauri command: emit a telemetry event from the frontend.
#[tauri::command]
fn track_event(state: tauri::State<'_, AppState>, event: TelemetryEvent) {
    state.telemetry.track(event);
}

/// Tauri command: check if the telemetry prompt has been shown.
#[tauri::command]
fn get_telemetry_prompted(state: tauri::State<'_, AppState>) -> bool {
    state.settings.lock().unwrap().telemetry_prompted
}

/// Tauri command: save the user's telemetry choice from the frontend prompt.
#[tauri::command]
fn set_telemetry_choice(state: tauri::State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut s = state.settings.lock().unwrap();
    s.telemetry_prompted = true;
    s.telemetry_enabled = enabled;
    s.save().map_err(|e| e.to_string())
}

/// Tauri command: toggle main window visibility (used by tray menu / IPC).
#[tauri::command]
fn toggle_window_visibility(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        match w.is_visible().unwrap_or(false) {
            true => { let _ = w.hide(); }
            false => { let _ = w.show(); let _ = w.set_focus(); }
        }
    }
}

/// Tauri command: show + focus the main window.
#[tauri::command]
fn show_window(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Tauri command: quit the app cleanly (used by tray menu).
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

// --- Plugin store IPC ---
//
// Thin wrappers around dofek::plugin::store::PluginStore. The store handles
// the heavy lifting (probe manifest, copy binary, chmod, xattr, append to
// plugins.toml) — these commands just translate path/string arguments and
// surface errors as plain strings for the frontend.

#[derive(serde::Serialize)]
pub struct PluginEntryView {
    pub name: String,
    pub binary_path: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

impl From<dofek::plugin::store::InstalledPlugin> for PluginEntryView {
    fn from(p: dofek::plugin::store::InstalledPlugin) -> Self {
        Self {
            name: p.name,
            binary_path: p.binary_path.to_string_lossy().to_string(),
            description: p.description,
            version: p.version,
            author: p.author,
            args: p.args,
            enabled: p.enabled,
        }
    }
}

#[tauri::command]
fn plugins_list() -> Result<Vec<PluginEntryView>, String> {
    let store = dofek::plugin::store::PluginStore::open().map_err(|e| e.to_string())?;
    let list = store.list().map_err(|e| e.to_string())?;
    Ok(list.into_iter().map(Into::into).collect())
}

#[tauri::command]
fn plugins_add(path: String, args: Vec<String>) -> Result<PluginEntryView, String> {
    let store = dofek::plugin::store::PluginStore::open().map_err(|e| e.to_string())?;
    let installed = store
        .add(std::path::Path::new(&path), args)
        .map_err(|e| format!("{e:#}"))?;
    Ok(installed.into())
}

#[tauri::command]
fn plugins_remove(name: String) -> Result<(), String> {
    let store = dofek::plugin::store::PluginStore::open().map_err(|e| e.to_string())?;
    store.remove(&name).map_err(|e| e.to_string())
}

#[tauri::command]
fn plugins_set_enabled(name: String, enabled: bool) -> Result<(), String> {
    let store = dofek::plugin::store::PluginStore::open().map_err(|e| e.to_string())?;
    store.set_enabled(&name, enabled).map_err(|e| e.to_string())
}

/// Native "choose plugin binary" file picker. Returns the selected absolute
/// path or `None` if the user cancelled. Doing this server-side keeps the
/// frontend JS plugin-agnostic — no @tauri-apps/plugin-dialog discovery dance.
#[tauri::command]
async fn plugins_pick_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Select a Dofek plugin binary")
        .pick_file(move |path| {
            let s = path.map(|p| p.to_string());
            let _ = tx.send(s);
        });
    rx.recv().map_err(|e| e.to_string())
}

/// Tauri command: kill a single process by PID.
#[tauri::command]
fn kill_process(state: tauri::State<'_, AppState>, pid: u32) -> Result<String, String> {
    kill_pid(pid)?;
    state.telemetry.track(TelemetryEvent::ProcessKill { success: true });
    Ok(format!("Killed PID {pid}"))
}

/// Tauri command: kill multiple processes by PID.
#[tauri::command]
fn kill_processes(state: tauri::State<'_, AppState>, pids: Vec<u32>) -> Result<String, String> {
    let mut killed = 0usize;
    let mut failed = 0usize;
    for pid in &pids {
        match kill_pid(*pid) {
            Ok(()) => killed += 1,
            Err(_) => failed += 1,
        }
    }
    state.telemetry.track(TelemetryEvent::ProcessKill { success: failed == 0 });
    let total = pids.len();
    if failed == 0 {
        Ok(format!("Killed all {total} processes"))
    } else {
        Err(format!("Killed {killed}/{total} ({failed} failed)"))
    }
}

#[cfg(windows)]
fn kill_pid(pid: u32) -> Result<(), String> {
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    use windows::Win32::Foundation::CloseHandle;
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
            .map_err(|e| format!("Access denied or not found: {e}"))?;
        let result = TerminateProcess(handle, 1);
        let _ = CloseHandle(handle);
        result.map_err(|e| format!("TerminateProcess failed: {e}"))
    }
}

#[cfg(unix)]
fn kill_pid(pid: u32) -> Result<(), String> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
        .map_err(|e| format!("kill({pid}, SIGTERM) failed: {e}"))
}

pub fn run() {
    env_logger::init();

    // Load config (same lookup as TUI)
    let cli = dofek::config::Cli { config: None, command: None };
    let config = Config::load(&cli).unwrap_or_default();

    // Load settings — telemetry respects first-run choice or config override
    let settings = UserSettings::load();
    let telemetry_enabled = settings.telemetry_enabled || config.telemetry.enabled;
    let telemetry = telemetry::spawn_telemetry(
        telemetry_enabled,
        &config.telemetry.endpoint,
        config.telemetry.flush_interval_secs,
        settings.anonymous_id.clone(),
    );
    telemetry.track(TelemetryEvent::SessionStart {
        interface: "gui".into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        os_version: dofek::os_version_string(),
    });
    let session_start = Instant::now();
    let shutdown_telemetry = telemetry.clone();

    // Spawn the data collector thread (reuses the exact same code as TUI).
    // The relay thread that fans snapshots into shared state + the tray icon
    // is spawned later inside `.setup(...)` because it needs an AppHandle.
    //
    // Floor the GUI's poll interval at 1000 ms regardless of config. Each
    // poll is a full `sysinfo::refresh_processes(All)` sweep (≥500 procs on a
    // typical Linux box), then the snapshot is cloned into shared state and
    // re-cloned per IPC call — at 500 ms cadence this put dofek-gui above
    // 100% CPU on its own. 1 Hz keeps the visualisation responsive without
    // burning a core to monitor the system. TUI keeps the configured rate.
    let gui_refresh_ms = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
        config.general.refresh_ms.max(1000),
    ));
    let data_rx = dofek::data::spawn_collector(config.clone(), std::sync::Arc::clone(&gui_refresh_ms));

    // Shared snapshot for Tauri commands
    let snapshot = Arc::new(Mutex::new(DataSnapshot::default()));

    let settings = Arc::new(Mutex::new(settings));

    let state = AppState {
        snapshot: Arc::clone(&snapshot),
        config,
        settings: Arc::clone(&settings),
        telemetry: telemetry.clone(),
    };

    let snapshot_for_setup = Arc::clone(&snapshot);
    let settings_for_setup = Arc::clone(&settings);
    let telemetry_for_setup = telemetry.clone();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    // Single-instance dedup on Windows + Linux (macOS handled by LaunchServices).
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }));
    }

    let close_settings = Arc::clone(&settings);
    let close_telemetry = telemetry.clone();

    let data_rx = Mutex::new(Some(data_rx));

    builder
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_gpu_info,
            get_platform_info,
            get_app_version,
            get_settings,
            save_settings,
            track_event,
            get_telemetry_prompted,
            set_telemetry_choice,
            kill_process,
            kill_processes,
            open_manual,
            check_for_update,
            open_url,
            toggle_window_visibility,
            show_window,
            quit_app,
            plugins_list,
            plugins_add,
            plugins_remove,
            plugins_set_enabled,
            plugins_pick_file,
        ])
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                let s = close_settings.lock().unwrap();
                if s.enable_tray && s.close_to_tray {
                    let _ = window.hide();
                    api.prevent_close();
                    close_telemetry.track(TelemetryEvent::WindowClosedToTray);
                }
            }
        })
        .setup(move |app| {
            use tauri::Manager;

            // Install the tray companion. Failures are logged and tolerated —
            // a missing tray shouldn't crash the app.
            let s = settings_for_setup.lock().unwrap().clone();
            if let Err(e) = tray::install(app, &s, telemetry_for_setup.clone()) {
                log::warn!("Failed to install tray icon: {e}");
            }

            // Honor start-in-tray.
            if s.enable_tray
                && s.start_in_tray
                && let Some(w) = app.get_webview_window("main")
            {
                let _ = w.hide();
            }

            // Move the data-relay loop here so it can update the tray.
            let app_handle = app.handle().clone();
            let snapshot_writer = Arc::clone(&snapshot_for_setup);
            let settings_for_relay = Arc::clone(&settings_for_setup);
            let rx = data_rx
                .lock()
                .unwrap()
                .take()
                .expect("data_rx claimed twice");
            std::thread::spawn(move || {
                use tauri::Emitter;
                for snap in rx {
                    {
                        let mut locked = snapshot_writer.lock().unwrap();
                        *locked = snap.clone();
                    }
                    // Push the snapshot to the frontend instead of having it
                    // poll get_snapshot every second. Removes one IPC round-trip
                    // per tick plus the JSON serialize/parse — the dominant
                    // remaining cost on WebKitGTK at 1 Hz. get_snapshot is kept
                    // around so the frontend can still hydrate on first paint.
                    let _ = app_handle.emit("dofek://snapshot", &snap);
                    let s = settings_for_relay.lock().unwrap().clone();
                    if s.enable_tray {
                        tray::update(&app_handle, &snap, &s);
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building Dofek GUI")
        .run(move |_app, event| {
            if let tauri::RunEvent::Exit = event {
                shutdown_telemetry.track(TelemetryEvent::SessionEnd {
                    duration_secs: session_start.elapsed().as_secs(),
                });
                shutdown_telemetry.shutdown();
            }
        });
}
