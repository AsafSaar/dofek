use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
}

/// Spawns a thread that reads crossterm events and sends them over the channel.
/// Returns the receiver. The thread runs until the channel is dropped.
#[allow(clippy::collapsible_match)]
pub fn spawn_event_reader(tick_rate: Duration) -> mpsc::Receiver<AppEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || loop {
        if event::poll(tick_rate).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    if tx.send(AppEvent::Key(key)).is_err() {
                        return;
                    }
                }
                Ok(Event::Resize(w, h)) => {
                    if tx.send(AppEvent::Resize(w, h)).is_err() {
                        return;
                    }
                }
                _ => {}
            }
        } else {
            // Tick event (no input within tick_rate)
            if tx.send(AppEvent::Tick).is_err() {
                return;
            }
        }
    });
    rx
}
