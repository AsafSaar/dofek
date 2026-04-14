use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};
use ratatui::Frame;

use crate::app::App;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" NET ", Style::default().fg(theme::NET_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("+ DISK ", Style::default().fg(theme::DISK_COLOR).add_modifier(Modifier::BOLD)),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_PANEL));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(3.min(inner.height.saturating_sub(2))),
        ])
        .split(inner);

    // Network stats
    let iface = app.data.network.interfaces.first();
    let (name, rx, tx) = if let Some(i) = iface {
        (
            truncate(&i.name, 20),
            format_bytes_rate(i.rx_bytes_per_sec),
            format_bytes_rate(i.tx_bytes_per_sec),
        )
    } else {
        ("No interface".to_string(), "0 B/s".to_string(), "0 B/s".to_string())
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(chunks[0]);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(&name, Style::default().fg(theme::TEXT_DIM)),
        ])),
        rows[0],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ↓ ", Style::default().fg(theme::NET_COLOR)),
            Span::styled(&rx, Style::default().fg(theme::TEXT_PRIMARY)),
            Span::raw("  "),
            Span::styled(" ↑ ", Style::default().fg(theme::NET_COLOR)),
            Span::styled(&tx, Style::default().fg(theme::TEXT_PRIMARY)),
        ])),
        rows[1],
    );

    // Sparkline for network rx
    let spark_data = app.history.net_rx.as_slice();
    let sparkline = Sparkline::default()
        .data(spark_data)
        .style(Style::default().fg(theme::NET_COLOR));
    f.render_widget(sparkline, chunks[1]);
}

fn format_bytes_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_073_741_824.0 {
        format!("{:.1} GB/s", bytes_per_sec / 1_073_741_824.0)
    } else if bytes_per_sec >= 1_048_576.0 {
        format!("{:.1} MB/s", bytes_per_sec / 1_048_576.0)
    } else if bytes_per_sec >= 1024.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1024.0)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}
