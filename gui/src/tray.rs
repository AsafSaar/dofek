//! System-tray / menu-bar companion.
//!
//! - Renders a 32×32 RGBA icon containing a sky-blue → red CPU sparkline; the
//!   icon is re-rendered every snapshot.
//! - Right-click menu: Show / Hide / Settings… / Quit.
//! - Left-click toggles window visibility.
//! - macOS menu-bar text (`CPU NN GPU NN`) is set via `set_title` when
//!   `settings.tray_show_text` is true; on Windows/Linux that call is a no-op.
//!
//! Reuses the single `DataSnapshot` stream — no second collector.

use std::collections::VecDeque;
use std::sync::Mutex;

use dofek::data::DataSnapshot;
use dofek::settings::UserSettings;
use dofek::telemetry::{TelemetryEvent, TelemetryHandle};
use tauri::image::Image;
use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke, Transform};

const ICON_SIZE: u32 = 32;
const HISTORY: usize = 32;

static CPU_SAMPLES: Mutex<VecDeque<f32>> = Mutex::new(VecDeque::new());

/// Push a CPU% sample into the rolling history.
pub fn record_cpu(sample: f32) {
    let mut q = CPU_SAMPLES.lock().unwrap();
    if q.len() == HISTORY {
        q.pop_front();
    }
    q.push_back(sample.clamp(0.0, 100.0));
}

/// Install the tray icon and menu, wire the click/menu handlers.
/// Skipped entirely if `settings.enable_tray == false`.
pub fn install(
    app: &tauri::App,
    settings: &UserSettings,
    telemetry: TelemetryHandle,
) -> tauri::Result<()> {
    if !settings.enable_tray {
        log::info!("Tray disabled by settings");
        return Ok(());
    }

    // Use MenuBuilder's text() chain instead of MenuItemBuilder + .item(&...).
    // The latter renders blank labels under libayatana-appindicator on Ubuntu
    // (Yaru / GNOME with the AppIndicator extension). Plain ASCII labels only —
    // skipping "Settings…" with U+2026 because some appindicator backends
    // truncate non-ASCII through the dbus path.
    let menu = MenuBuilder::new(app)
        .text("tray.show", "Show window")
        .text("tray.hide", "Hide window")
        .separator()
        .text("tray.settings", "Settings")
        .separator()
        .text("tray.quit", "Quit dofek")
        .build()?;

    let initial_icon = render_sparkline_icon(&[]);

    let telemetry_for_clicks = telemetry.clone();

    let _tray = TrayIconBuilder::with_id("main")
        .icon(initial_icon)
        .tooltip("dofek — system monitor")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                telemetry_for_clicks.track(TelemetryEvent::TrayIconClicked);
                toggle_window(tray.app_handle());
            }
        })
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            telemetry.track(TelemetryEvent::TrayMenuItemSelected {
                item: id.to_string(),
            });
            match id {
                "tray.show" => show_window(app),
                "tray.hide" => hide_window(app),
                "tray.settings" => {
                    show_window(app);
                    let _ = app.emit("dofek://open-settings", ());
                }
                "tray.quit" => app.exit(0),
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

/// Re-render the tray icon based on the latest snapshot, and (optionally) the
/// macOS menu-bar text. Should be called from the snapshot relay loop.
pub fn update(app: &AppHandle, snap: &DataSnapshot, settings: &UserSettings) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };

    record_cpu(snap.cpu.total_load);
    let samples: Vec<f32> = CPU_SAMPLES.lock().unwrap().iter().copied().collect();

    if let Err(e) = tray.set_icon(Some(render_sparkline_icon(&samples))) {
        log::debug!("tray.set_icon failed: {e}");
    }

    let cpu = snap.cpu.total_load.round() as i32;
    let gpu = snap
        .gpus
        .first()
        .map(|g| g.utilization.round() as i32)
        .unwrap_or(0);
    let tooltip = if snap.gpus.is_empty() {
        format!("dofek · CPU {cpu}%")
    } else {
        format!("dofek · CPU {cpu}% · GPU {gpu}%")
    };
    let _ = tray.set_tooltip(Some(&tooltip));

    let title = if settings.tray_show_text {
        Some(if snap.gpus.is_empty() {
            format!("CPU {cpu}")
        } else {
            format!("CPU {cpu} GPU {gpu}")
        })
    } else {
        None
    };
    let _ = tray.set_title(title.as_deref());
}

fn toggle_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        match w.is_visible().unwrap_or(false) {
            true => {
                let _ = w.hide();
            }
            false => {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
    }
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn hide_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

/// Draw the 32×32 RGBA tray icon. Stroke ramps from sky-blue at 0% to red at 100%
/// with the latest sample. An empty/single-sample buffer renders a flat baseline.
fn render_sparkline_icon(samples: &[f32]) -> Image<'static> {
    let mut pixmap = Pixmap::new(ICON_SIZE, ICON_SIZE).expect("alloc tray pixmap");

    // Subtle dark backplate with rounded feel — keeps the line visible on both
    // light and dark menu bars.
    let mut bg = Paint::default();
    bg.set_color_rgba8(15, 23, 42, 0); // fully transparent — let OS theme through
    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    let last = samples.last().copied().unwrap_or(0.0);
    let stroke_color = ramp_color(last);

    let mut paint = Paint::default();
    paint.set_color_rgba8(stroke_color.0, stroke_color.1, stroke_color.2, 255);
    paint.anti_alias = true;

    let stroke = Stroke {
        width: 1.6,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Stroke::default()
    };

    if samples.len() < 2 {
        // Flat baseline at 75% height (looks like a settled idle line)
        let mut pb = PathBuilder::new();
        pb.move_to(2.0, ICON_SIZE as f32 * 0.75);
        pb.line_to(ICON_SIZE as f32 - 2.0, ICON_SIZE as f32 * 0.75);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    } else {
        let n = samples.len();
        let w = ICON_SIZE as f32 - 2.0;
        let h = ICON_SIZE as f32 - 4.0;
        let mut pb = PathBuilder::new();
        for (i, &v) in samples.iter().enumerate() {
            let x = 1.0 + (i as f32 / (n - 1) as f32) * w;
            let y = 2.0 + (1.0 - v.clamp(0.0, 100.0) / 100.0) * h;
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
    let _ = bg; // bg paint reserved for future variants

    Image::new_owned(pixmap.take(), ICON_SIZE, ICON_SIZE)
}

/// Sky-blue → amber → red ramp keyed on CPU%.
fn ramp_color(pct: f32) -> (u8, u8, u8) {
    let p = pct.clamp(0.0, 100.0) / 100.0;
    if p < 0.5 {
        // 0..50: sky-blue → emerald (idle to working)
        let t = p / 0.5;
        let r = lerp(0x38, 0x34, t);
        let g = lerp(0xBD, 0xD3, t);
        let b = lerp(0xF8, 0x99, t);
        (r, g, b)
    } else if p < 0.8 {
        // 50..80: emerald → amber
        let t = (p - 0.5) / 0.3;
        let r = lerp(0x34, 0xFB, t);
        let g = lerp(0xD3, 0xBF, t);
        let b = lerp(0x99, 0x24, t);
        (r, g, b)
    } else {
        // 80..100: amber → red
        let t = (p - 0.8) / 0.2;
        let r = lerp(0xFB, 0xF8, t);
        let g = lerp(0xBF, 0x71, t);
        let b = lerp(0x24, 0x71, t);
        (r, g, b)
    }
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let af = a as f32;
    let bf = b as f32;
    (af + (bf - af) * t.clamp(0.0, 1.0)).round() as u8
}
