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
use std::time::{Duration, Instant};

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

/// Three-way display selector: sparkline icon only, sparkline + text, or text
/// only. Text-only is a macOS-only affordance — Windows/Linux system trays
/// don't render a title, so on those platforms we fall back to chart-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMode {
    Chart,
    ChartAndText,
    Text,
}

impl TrayMode {
    /// Parse the persisted setting. Falls back to the legacy
    /// `tray_show_text` boolean when `tray_display_mode` is unset/unrecognised
    /// so users upgrading from <1.4 don't lose their previous choice.
    fn from_settings(s: &UserSettings) -> Self {
        match s.tray_display_mode.as_str() {
            "chart" => TrayMode::Chart,
            "text" => TrayMode::Text,
            "chart+text" => TrayMode::ChartAndText,
            _ => {
                if s.tray_show_text {
                    TrayMode::ChartAndText
                } else {
                    TrayMode::Chart
                }
            }
        }
    }

    fn shows_chart(self) -> bool {
        // Text-only is honored on macOS only; elsewhere we still draw the
        // sparkline since title text is invisible on those platforms.
        if cfg!(target_os = "macos") {
            matches!(self, TrayMode::Chart | TrayMode::ChartAndText)
        } else {
            true
        }
    }

    fn shows_text(self) -> bool {
        matches!(self, TrayMode::ChartAndText | TrayMode::Text)
    }
}
/// Cap tray repaints at ~1 Hz. The relay loop fires at the data-collector
/// cadence (default 500 ms); each repaint encodes a PNG, writes it to /tmp,
/// and dispatches a dbus NewIcon notification — pegging dofek-gui's CPU at
/// 100%+ on Linux. 1 Hz is plenty for a sparkline meant to be read at a glance.
const TRAY_PAINT_MIN_INTERVAL: Duration = Duration::from_millis(1000);

static CPU_SAMPLES: Mutex<VecDeque<f32>> = Mutex::new(VecDeque::new());
static LAST_TRAY_PAINT: Mutex<Option<Instant>> = Mutex::new(None);
/// Last (rounded CPU%, rounded GPU%) pushed to tooltip/title. Skip dbus
/// roundtrips when both numbers are unchanged.
static LAST_TRAY_TEXT: Mutex<Option<(i32, i32)>> = Mutex::new(None);
/// Last applied display mode. We track it so a mode flip (e.g. chart → text)
/// repaints the icon immediately even if CPU/GPU values haven't moved.
static LAST_TRAY_MODE: Mutex<Option<TrayMode>> = Mutex::new(None);

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

    // The menu event handler is registered at build time on every platform.
    // It dispatches whichever menu is *currently attached* to the tray, so
    // attaching the menu later (Linux path below) still routes clicks here.
    let telemetry_for_clicks = telemetry.clone();
    let menu_handler = move |app: &AppHandle, event: tauri::menu::MenuEvent| {
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
    };

    // Windows + macOS: build the menu eagerly and attach at construction.
    // Linux: skip — we attach via set_menu after a delay (see below).
    #[cfg(not(target_os = "linux"))]
    let menu_at_build = build_tray_menu(app)?;

    let initial_icon = render_sparkline_icon(&[]);

    #[allow(unused_mut)] // .menu(...) is platform-gated below
    let mut builder = TrayIconBuilder::with_id("main")
        .icon(initial_icon)
        .tooltip("Dofek — system monitor")
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
        .on_menu_event(menu_handler);

    #[cfg(not(target_os = "linux"))]
    {
        builder = builder.menu(&menu_at_build);
    }

    let _tray = builder.build(app)?;

    // Linux: defer the menu attach to dodge the libayatana-appindicator
    // dbusmenu race. The GNOME AppIndicator extension reads
    // /com/canonical/dbusmenu shortly after the indicator transitions to
    // Active; if the menu is attached at construction the extension caches
    // a blank model before child widgets are realised. Sleeping ~1.5 s
    // before calling set_menu gives the dbus exporter time to settle, and
    // the menu must be built on the gtk main thread (run_on_main_thread).
    #[cfg(target_os = "linux")]
    {
        let app_handle = app.handle().clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            let app_for_main = app_handle.clone();
            if let Err(e) = app_handle.run_on_main_thread(move || {
                match build_tray_menu(&app_for_main) {
                    Ok(menu) => {
                        if let Some(tray) = app_for_main.tray_by_id("main") {
                            if let Err(e) = tray.set_menu(Some(menu)) {
                                log::warn!("delayed tray.set_menu failed: {e}");
                            }
                        }
                    }
                    Err(e) => log::warn!("delayed tray menu build failed: {e}"),
                }
            }) {
                log::warn!("run_on_main_thread for tray menu failed: {e}");
            }
        });
    }

    Ok(())
}

/// Build the static tray menu. Same labels on every platform; ASCII-only
/// since some appindicator dbus backends mishandle non-ASCII (e.g. U+2026).
fn build_tray_menu<R: tauri::Runtime, M: tauri::Manager<R>>(
    app: &M,
) -> tauri::Result<tauri::menu::Menu<R>> {
    MenuBuilder::new(app)
        .text("tray.show", "Show window")
        .text("tray.hide", "Hide window")
        .separator()
        .text("tray.settings", "Settings")
        .separator()
        .text("tray.quit", "Quit Dofek")
        .build()
}

/// Re-render the tray icon based on the latest snapshot, and (optionally) the
/// macOS menu-bar text. Should be called from the snapshot relay loop.
pub fn update(app: &AppHandle, snap: &DataSnapshot, settings: &UserSettings) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };

    record_cpu(snap.cpu.total_load);

    let mode = TrayMode::from_settings(settings);

    // A mode change (chart ↔ text) needs to repaint the icon immediately so
    // the user sees the toggle land instead of waiting for the next throttle
    // window. We compare against the last mode we applied.
    let mode_changed = {
        let mut last = LAST_TRAY_MODE.lock().unwrap();
        let changed = *last != Some(mode);
        *last = Some(mode);
        changed
    };

    // Throttle the icon repaint — see TRAY_PAINT_MIN_INTERVAL note above. The
    // CPU sample is still recorded every snapshot so the sparkline ring buffer
    // stays accurate; we just don't re-encode the PNG every time.
    let now = Instant::now();
    let should_paint = mode_changed || {
        let mut last = LAST_TRAY_PAINT.lock().unwrap();
        let due = last.is_none_or(|t| now.duration_since(t) >= TRAY_PAINT_MIN_INTERVAL);
        if due {
            *last = Some(now);
        }
        due
    };
    if should_paint {
        // For text-only on macOS we pass None so NSStatusItem actually drops
        // the image rather than rendering a 32×32 transparent gap. A blank
        // pixmap looked right in theory but left a phantom indent before the
        // text, which is what made the mode flip seem "stuck".
        let icon: Option<Image> = if mode.shows_chart() {
            let samples: Vec<f32> = CPU_SAMPLES.lock().unwrap().iter().copied().collect();
            Some(render_sparkline_icon(&samples))
        } else {
            None
        };
        if let Err(e) = tray.set_icon(icon) {
            log::debug!("tray.set_icon failed: {e}");
        }
    }

    // Tooltip + title also go through dbus (`set_label` on Linux). Skip them
    // unless the rounded values changed — keeps the dofek-gui process quiet
    // when the system is idle. A mode flip also forces an update so toggling
    // text on/off applies immediately.
    let cpu = snap.cpu.total_load.round() as i32;
    let gpu = snap
        .gpus
        .first()
        .map(|g| g.utilization.round() as i32)
        .unwrap_or(0);
    let mut last = LAST_TRAY_TEXT.lock().unwrap();
    let values_changed = match *last {
        Some((c, g)) => c != cpu || g != gpu,
        None => true,
    };
    if !values_changed && !mode_changed {
        return;
    }
    *last = Some((cpu, gpu));
    drop(last);

    let tooltip = if snap.gpus.is_empty() {
        format!("Dofek · CPU {cpu}%")
    } else {
        format!("Dofek · CPU {cpu}% · GPU {gpu}%")
    };
    let _ = tray.set_tooltip(Some(&tooltip));

    // On macOS, `set_title(None)` does not reliably clear the NSStatusItem
    // title — Tauri's bridge leaves the previous text in place. Passing an
    // empty string forces the platform to repaint the slot as empty, which is
    // what "Chart only" / no-text actually wants.
    let title: String = if mode.shows_text() {
        if snap.gpus.is_empty() {
            format!("CPU {cpu}")
        } else {
            format!("CPU {cpu} GPU {gpu}")
        }
    } else {
        String::new()
    };
    let _ = tray.set_title(Some(&title));
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
