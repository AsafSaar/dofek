use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme;

pub fn render(f: &mut Frame) {
    let area = f.area();

    let width = 46.min(area.width.saturating_sub(4));
    let height = 16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" about ")
        .title_style(Style::default().fg(theme::ACCENT_INDIGO).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::ACCENT_INDIGO))
        .style(Style::default().bg(theme::BG_PANEL));

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("dofek ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(concat!("v", env!("CARGO_PKG_VERSION")), Style::default().fg(theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Terminal / GUI, AI-aware system monitor",
            Style::default().fg(theme::TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "dofek.dev",
            Style::default().fg(theme::CPU_COLOR),
        )),
        Line::from(Span::styled(
            "linkedin.com/in/asafsaar",
            Style::default().fg(theme::CPU_COLOR),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Built by Asaf Saar · © 2026 MIT",
            Style::default().fg(theme::TEXT_DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "press any key to close",
            Style::default().fg(theme::TEXT_DIM),
        )),
    ];

    let para = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);

    f.render_widget(para, popup);
}
