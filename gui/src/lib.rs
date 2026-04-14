use std::sync::{Arc, Mutex};
use std::time::Instant;

use dofek::config::Config;
use dofek::data::DataSnapshot;
use dofek::settings::UserSettings;
use dofek::telemetry::{self, TelemetryEvent, TelemetryHandle};

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

/// Tauri command: returns the current user settings.
#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> UserSettings {
    state.settings.lock().unwrap().clone()
}

/// Tauri command: saves user settings to disk.
#[tauri::command]
fn save_settings(state: tauri::State<'_, AppState>, settings: UserSettings) -> Result<(), String> {
    let mut current = state.settings.lock().unwrap();
    *current = settings.clone();
    settings.save().map_err(|e| e.to_string())
}

/// Tauri command: emit a telemetry event from the frontend.
#[tauri::command]
fn track_event(state: tauri::State<'_, AppState>, event: TelemetryEvent) {
    state.telemetry.track(event);
}

pub fn run() {
    env_logger::init();

    // Load config (same lookup as TUI)
    let cli = dofek::config::Cli { config: None };
    let config = Config::load(&cli).unwrap_or_default();

    // Load settings and spawn telemetry
    let settings = UserSettings::load();
    let telemetry = telemetry::spawn_telemetry(
        config.telemetry.enabled,
        &config.telemetry.endpoint,
        config.telemetry.flush_interval_secs,
        settings.anonymous_id.clone(),
    );
    telemetry.track(TelemetryEvent::SessionStart {
        interface: "gui".into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        os_version: std::env::var("OS").unwrap_or_default(),
    });
    let session_start = Instant::now();
    let shutdown_telemetry = telemetry.clone();

    // Spawn the data collector thread (reuses the exact same code as TUI)
    let data_rx = dofek::data::spawn_collector(config.clone());

    // Shared snapshot for Tauri commands
    let snapshot = Arc::new(Mutex::new(DataSnapshot::default()));
    let snapshot_writer = Arc::clone(&snapshot);

    // Background thread: receives snapshots from collector, stores latest
    std::thread::spawn(move || {
        for snap in data_rx {
            let mut locked = snapshot_writer.lock().unwrap();
            *locked = snap;
        }
    });

    let settings = Arc::new(Mutex::new(settings));

    let state = AppState {
        snapshot,
        config,
        settings,
        telemetry,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(state)
        .invoke_handler(tauri::generate_handler![get_snapshot, get_gpu_info, get_settings, save_settings, track_event])
        .build(tauri::generate_context!())
        .expect("error building dofek GUI")
        .run(move |_app, event| {
            if let tauri::RunEvent::Exit = event {
                shutdown_telemetry.track(TelemetryEvent::SessionEnd {
                    duration_secs: session_start.elapsed().as_secs(),
                });
                shutdown_telemetry.shutdown();
            }
        });
}
