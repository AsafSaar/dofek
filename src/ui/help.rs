use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme;

pub fn render(f: &mut Frame, telemetry_enabled: bool) {
    let area = f.area();

    let width = 46.min(area.width.saturating_sub(4));
    // 21 help lines + telemetry + footer + spacing + 2 for border
    let height = 27u16.min(area.height.saturating_sub(4));
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

    let telem_status = if telemetry_enabled { "ON" } else { "OFF" };
    let telem_color = if telemetry_enabled { theme::MEM_COLOR } else { theme::TEXT_DIM };

    let lines = vec![
        Line::from(""),
        help_line("q", "Quit"),
        help_line("tab", "Cycle sort column"),
        help_line("p", "Full-screen processes"),
        help_line("↑↓/j/k", "Navigate process list"),
        help_line("/", "Search processes by name"),
        help_line("del/x", "Kill selected process"),
        help_line("X", "Kill all matching"),
        help_line("t", "Toggle tree/flat view"),
        help_line("→/←", "Expand/collapse group"),
        help_line("c/g/m/n", "CPU / GPU / MEM / NET"),
        help_line("d", "Disk I/O chart"),
        help_line("1-4", "Filter ALL/AI/DEV/WATCH"),
        help_line("esc", "Return to dashboard"),
        help_line("+/-", "Adjust refresh rate"),
        help_line("[ ]", "Resize chart/watchlist"),
        help_line("h", "Toggle horizon chart"),
        help_line("s", "Save snapshot"),
        help_line("u", "Check for updates"),
        help_line("a", "About dofek"),
        help_line("?", "Toggle this help"),
        Line::from(""),
        Line::from(vec![
            Span::styled("        T  ", Style::default().fg(theme::ACCENT_INDIGO).add_modifier(Modifier::BOLD)),
            Span::styled("Telemetry: ", Style::default().fg(theme::TEXT_PRIMARY)),
            Span::styled(telem_status, Style::default().fg(telem_color).add_modifier(Modifier::BOLD)),
        ]),
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
