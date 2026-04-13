use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme;

pub fn render(f: &mut Frame) {
    let area = f.area();

    // Center the help overlay
    let width = 50.min(area.width.saturating_sub(4));
    let height = 18.min(area.height.saturating_sub(4));
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
        help_line("p", "Focus process table"),
        help_line("g", "Focus GPU panel"),
        help_line("c", "Focus CPU panel"),
        help_line("m", "Focus memory panel"),
        help_line("esc", "Return to dashboard"),
        help_line("+/-", "Adjust refresh rate"),
        help_line("[ ]", "Resize chart/watchlist"),
        help_line("s", "Save snapshot"),
        help_line("?", "Toggle this help"),
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

fn help_line(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {:>6} ", key), Style::default().fg(theme::ACCENT_INDIGO).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {}", desc), Style::default().fg(theme::TEXT_PRIMARY)),
    ])
}
