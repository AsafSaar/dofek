use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme;

pub fn render(f: &mut Frame) {
    let area = f.area();

    let width = 44.min(area.width.saturating_sub(4));
    let height = 19.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" dofek — help ")
        .title_style(Style::default().fg(theme::ACCENT_INDIGO).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::ACCENT_INDIGO))
        .style(Style::default().bg(theme::BG_PANEL));

    let lines = vec![
        Line::from(""),
        help_line("q", "Quit"),
        help_line("tab", "Cycle sort column"),
        help_line("p", "Full-screen processes"),
        help_line("c/g/m/n", "CPU / GPU / MEM / NET"),
        help_line("1-4", "Filter ALL/AI/DEV/WATCH"),
        help_line("esc", "Return to dashboard"),
        help_line("+/-", "Adjust refresh rate"),
        help_line("[ ]", "Resize chart/watchlist"),
        help_line("s", "Save snapshot"),
        help_line("a", "About dofek"),
        help_line("?", "Toggle this help"),
        Line::from(""),
        Line::from(vec![
            Span::styled("      press any key to close", Style::default().fg(theme::TEXT_DIM)),
        ]),
    ];

    let para = Paragraph::new(lines).block(block);

    f.render_widget(para, popup);
}

fn help_line(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:>7}  ", key),
            Style::default().fg(theme::ACCENT_INDIGO).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            desc.to_string(),
            Style::default().fg(theme::TEXT_PRIMARY),
        ),
    ])
}
