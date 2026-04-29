#![allow(dead_code)]

mod app;
mod config;
mod data;
mod event;
mod plugin;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};

use dofek::telemetry::{self, TelemetryEvent};

use app::App;
use config::{Cli, Config};
use event::AppEvent;

/// Returns true if the terminal advertises 24-bit RGB color support.
///
/// We trust `COLORTERM=truecolor` / `COLORTERM=24bit` because every modern
/// truecolor terminal sets one of those (it's the de-facto convention) and
/// the few legacy terminals that mishandle truecolor SGR (Apple Terminal,
/// older xterm, basic SSH-as-vt100 sessions) don't.
///
/// `NO_COLOR` is also respected — if set, we treat the terminal as
/// non-truecolor so the user gets the warning and can choose to abort.
fn truecolor_supported() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    matches!(
        std::env::var("COLORTERM").as_deref(),
        Ok("truecolor") | Ok("24bit")
    )
}

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    let config = Config::load(&cli)?;

    // Pre-flight terminal capability check. Dofek's trading-terminal palette is
    // built on 24-bit RGB; terminals without truecolor support (notably Apple
    // Terminal.app — known broken since at least 2014) misparse the
    // `ESC[38;2;R;G;Bm` SGR sequences and render the whole UI in neon magenta
    // and red. Catch this *before* we take over the screen so the user has
    // somewhere readable to land after Ctrl+C, and so the warning isn't
    // overwritten by alt-screen takeover.
    if !truecolor_supported() {
        let term = std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "unknown".into());
        eprintln!();
        eprintln!("\x1b[33m! Dofek: terminal '{term}' does not advertise truecolor (COLORTERM unset).\x1b[0m");
        eprintln!("\x1b[33m  The trading-terminal palette uses 24-bit RGB; without it, panel");
        eprintln!("  backgrounds will render as miscolored blocks (Apple Terminal.app is the");
        eprintln!("  most common case). For correct rendering, run dofek-tui in iTerm2,");
        eprintln!("  WezTerm, Ghostty, Alacritty, or Kitty.\x1b[0m");
        eprintln!();
        eprintln!("  Starting anyway in 3 seconds — press Ctrl+C to abort.");
        std::thread::sleep(Duration::from_secs(3));
    }

    // Shared polling cadence: seeded from dofek.toml, mutated at runtime by
    // App's `+`/`-` keys, read on every collector iteration.
    let refresh_ms = Arc::new(AtomicU64::new(config.general.refresh_ms));

    // Spawn data collector thread
    let data_rx = data::spawn_collector(config.clone(), Arc::clone(&refresh_ms));

    // Spawn event reader thread
    let tick_rate = Duration::from_millis(16); // ~60fps event polling
    let event_rx = event::spawn_event_reader(tick_rate);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Check terminal dimensions and suggest font size if too small
    {
        let size = terminal.size()?;
        if size.width < 120 || size.height < 30 {
            use ratatui::style::{Color, Style};
            use ratatui::text::{Line, Span};
            use ratatui::widgets::Paragraph;
            use ratatui::layout::Alignment;
            use crossterm::event::{poll, read};

            for remaining in (1..=5).rev() {
                terminal.draw(|f| {
                    let area = f.area();
                    let msg = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "Dofek",
                            Style::default().fg(Color::Rgb(56, 189, 248)).add_modifier(ratatui::style::Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            format!("Terminal size: {}×{}", size.width, size.height),
                            Style::default().fg(Color::Rgb(148, 163, 184)),
                        )),
                        Line::from(Span::styled(
                            "Best viewed at 160+ columns. Try font size 9-10pt.",
                            Style::default().fg(Color::Rgb(148, 163, 184)),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            format!("Starting in {remaining}s — press any key to start now"),
                            Style::default().fg(Color::Rgb(61, 80, 112)),
                        )),
                    ])
                    .alignment(Alignment::Center);
                    f.render_widget(msg, area);
                })?;

                // Wait 1 second, but break immediately on any keypress
                if poll(Duration::from_secs(1))? {
                    let _ = read()?; // consume the event
                    break;
                }
            }
        }
    }

    // Load settings and prompt for telemetry on first run
    let mut settings = dofek::settings::UserSettings::load();
    if !settings.telemetry_prompted {
        use ratatui::style::{Color, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;
        use ratatui::layout::Alignment;
        use crossterm::event::{poll, read, Event, KeyCode as EK};

        let mut answered = false;
        while !answered {
            terminal.draw(|f| {
                let area = f.area();
                let msg = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "dofek",
                        Style::default().fg(Color::Rgb(56, 189, 248)).add_modifier(ratatui::style::Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Help improve Dofek by sharing anonymous usage data?",
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("What's collected: ", Style::default().fg(Color::Rgb(148, 163, 184))),
                        Span::styled("session duration, feature usage, GPU detection path", Style::default().fg(Color::Rgb(100, 116, 139))),
                    ]),
                    Line::from(vec![
                        Span::styled("Never collected: ", Style::default().fg(Color::Rgb(148, 163, 184))),
                        Span::styled("process names, hostnames, IPs, or system metrics", Style::default().fg(Color::Rgb(100, 116, 139))),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  y ", Style::default().fg(Color::Rgb(52, 211, 153)).add_modifier(ratatui::style::Modifier::BOLD)),
                        Span::styled("yes, share anonymous data    ", Style::default().fg(Color::Rgb(148, 163, 184))),
                        Span::styled("  n ", Style::default().fg(Color::Rgb(248, 113, 113)).add_modifier(ratatui::style::Modifier::BOLD)),
                        Span::styled("no thanks", Style::default().fg(Color::Rgb(148, 163, 184))),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "You can change this later in dofek.toml → [telemetry]",
                        Style::default().fg(Color::Rgb(61, 80, 112)),
                    )),
                ])
                .alignment(Alignment::Center);
                f.render_widget(msg, area);
            })?;

            if poll(Duration::from_millis(100))?
                && let Event::Key(key) = read()? {
                    match key.code {
                        EK::Char('y') | EK::Char('Y') => {
                            settings.telemetry_enabled = true;
                            answered = true;
                        }
                        EK::Char('n') | EK::Char('N') | EK::Esc => {
                            settings.telemetry_enabled = false;
                            answered = true;
                        }
                        _ => {}
                    }
                }
        }
        settings.telemetry_prompted = true;
        let _ = settings.save();
        terminal.clear()?;
    }

    // Telemetry is enabled if the user opted in OR if the config explicitly enables it
    let telemetry_enabled = settings.telemetry_enabled || config.telemetry.enabled;
    let telemetry = telemetry::spawn_telemetry(
        telemetry_enabled,
        &config.telemetry.endpoint,
        config.telemetry.flush_interval_secs,
        settings.anonymous_id.clone(),
    );
    telemetry.track(TelemetryEvent::SessionStart {
        interface: "tui".into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        os_version: dofek::os_version_string(),
    });
    let session_start = Instant::now();

    let mut app = App::new(config, telemetry.clone(), Arc::clone(&refresh_ms));

    // Restore saved user settings
    app.apply_settings(&settings);

    // Opt-in startup update check. Runs in the background; the result is
    // surfaced only when the user opens the update overlay (via `u`) or — if
    // a newer version is found — by auto-popping the overlay once it lands.
    if settings.check_updates_on_startup {
        app.trigger_update_check();
    }
    let mut update_auto_shown = false;

    // Telemetry: emit GPU path once, heartbeat periodically
    let mut gpu_path_emitted = false;
    let mut snapshot_count: u64 = 0;

    // Main loop
    loop {
        // Receive data snapshots (non-blocking)
        while let Ok(snapshot) = data_rx.try_recv() {
            app.update_data(snapshot);
            snapshot_count += 1;

            // Emit GPU detection path once after first real snapshot
            if !gpu_path_emitted {
                let path = if app.data.nvml_available { "nvml" }
                    else if app.data.lhm_connected { "lhm" }
                    else { "none" };
                telemetry.track(TelemetryEvent::GpuPath {
                    path: path.into(),
                    device_count: app.data.gpus.len(),
                    device_names: app.data.gpus.iter().map(|g| g.name.clone()).collect(),
                });
                gpu_path_emitted = true;
            }

            // Heartbeat every ~300 snapshots (~2.5 min at 500ms refresh)
            if snapshot_count.is_multiple_of(300) {
                telemetry.track(TelemetryEvent::Heartbeat {
                    current_tab: format!("{:?}", app.chart_tab).to_lowercase(),
                    process_count: app.data.processes.len(),
                    plugin_count: app.data.plugin_statuses.len(),
                });
            }
        }

        // Render
        terminal.draw(|f| {
            ui::render(f, &app);
        })?;

        // Handle events (blocking with tick_rate timeout via the event thread)
        match event_rx.recv_timeout(Duration::from_millis(16)) {
            Ok(AppEvent::Key(key)) => {
                app.handle_key(key);
            }
            Ok(AppEvent::Resize(_, _)) => {
                // Terminal will auto-resize on next draw
            }
            Ok(AppEvent::Tick) | Err(_) => {
                // Normal tick, continue loop
            }
        }

        // If a startup update check finds a newer release, surface the
        // overlay once. The user can dismiss it like any other overlay; we
        // never re-pop it within the same session.
        if settings.check_updates_on_startup && !update_auto_shown {
            let state = app.update_state.lock().unwrap().clone();
            if let app::UpdateState::Ready(ref info) = state
                && info.is_newer
                && !app.show_help
                && !app.show_about
            {
                app.show_update = true;
                update_auto_shown = true;
            } else if matches!(state, app::UpdateState::Error(_) | app::UpdateState::Ready(_)) {
                // Resolved without a newer version (or hit an error) — don't
                // bother the user.
                update_auto_shown = true;
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Flush telemetry before exit
    telemetry.track(TelemetryEvent::SessionEnd {
        duration_secs: session_start.elapsed().as_secs(),
    });
    telemetry.shutdown();

    // Save user settings before exit
    if let Err(e) = app.to_settings(&settings).save() {
        log::warn!("Failed to save settings: {e}");
    }

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
