use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::ui::theme;

/// Render the bottom status bar (1 line): keybindings + refresh rate.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(12),
        ])
        .split(area);

    let keys = vec![
        ("q", "quit"),
        ("tab", "sort"),
        ("p", "proc"),
        ("c", "cpu"),
        ("g", "gpu"),
        ("m", "mem"),
        ("n", "net"),
        ("h", "horizon"),
        ("1-4", "filter"),
        ("[]", "resize"),
        ("s", "snapshot"),
        ("a", "about"),
        ("?", "help"),
    ];

    let mut spans: Vec<Span> = Vec::new();
    for (key, label) in &keys {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{label} "),
            Style::default().fg(theme::TEXT_DIM),
        ));
    }

    let left = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(theme::BG_PRIMARY));
    f.render_widget(left, chunks[0]);

    let rate = format!("{}ms ", app.refresh_ms);
    let right = Paragraph::new(Line::from(vec![
        Span::styled(rate, Style::default().fg(theme::TEXT_DIM)),
    ]))
    .alignment(ratatui::layout::Alignment::Right)
    .style(Style::default().bg(theme::BG_PRIMARY));
    f.render_widget(right, chunks[1]);
}
