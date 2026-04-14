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
                            "dofek",
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
