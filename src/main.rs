#![allow(dead_code)]

mod app;
mod config;
mod data;
mod event;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

use app::App;
use config::{Cli, Config};
use event::AppEvent;

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    let config = Config::load(&cli)?;

    // Spawn data collector thread
    let data_rx = data::spawn_collector(config.clone());

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

    let mut app = App::new(config);

    // Restore saved user settings
    let settings = dofek::settings::UserSettings::load();
    app.apply_settings(&settings);

    // Main loop
    loop {
        // Receive data snapshots (non-blocking)
        while let Ok(snapshot) = data_rx.try_recv() {
            app.update_data(snapshot);
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

        if app.should_quit {
            break;
        }
    }

    // Save user settings before exit
    if let Err(e) = app.to_settings().save() {
        log::warn!("Failed to save settings: {e}");
    }

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
