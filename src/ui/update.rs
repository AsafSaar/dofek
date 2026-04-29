use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, UpdateState};
use crate::ui::theme;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let width = 60u16.min(area.width.saturating_sub(4));
    let height = 18u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup);

    let state = app.update_state.lock().unwrap().clone();

    let (title, accent, body) = match state {
        UpdateState::Idle => (
            " update ",
            theme::ACCENT_INDIGO,
            vec![
                Line::from(""),
                centered("Press u to check for updates.", theme::TEXT_SECONDARY),
            ],
        ),
        UpdateState::Checking => (
            " update — checking ",
            theme::ACCENT_INDIGO,
            vec![
                Line::from(""),
                centered("Querying GitHub…", theme::TEXT_SECONDARY),
            ],
        ),
        UpdateState::Ready(info) if info.is_newer => (
            " update available ",
            theme::MEM_COLOR,
            ready_lines(&info, true),
        ),
        UpdateState::Ready(info) => (
            " up to date ",
            theme::ACCENT_INDIGO,
            ready_lines(&info, false),
        ),
        UpdateState::Error(msg) => (
            " update — error ",
            theme::CRIT_COLOR,
            vec![
                Line::from(""),
                centered("Update check failed:", theme::TEXT_PRIMARY),
                Line::from(""),
                centered(&msg, theme::TEXT_SECONDARY),
            ],
        ),
    };

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(theme::BG_PANEL));

    let mut lines = body;
    lines.push(Line::from(""));
    lines.push(centered("press any key to close", theme::TEXT_DIM));

    let para = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(para, popup);
}

fn ready_lines(info: &dofek::update::UpdateInfo, is_newer: bool) -> Vec<Line<'static>> {
    let headline = if is_newer {
        format!("Dofek v{} is available", info.latest)
    } else {
        format!("You're on the latest release (v{})", info.current)
    };
    let headline_color = if is_newer { theme::MEM_COLOR } else { theme::ACCENT_INDIGO };

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            headline,
            Style::default().fg(headline_color).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("current ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled(format!("v{}", info.current), Style::default().fg(theme::TEXT_PRIMARY)),
            Span::styled("    latest ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled(format!("v{}", info.latest), Style::default().fg(theme::TEXT_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            info.url.clone(),
            Style::default().fg(theme::CPU_COLOR),
        )),
    ];
    if is_newer && !info.notes.is_empty() {
        lines.push(Line::from(""));
        // First line of release notes only — overlay is small.
        let snippet: String = info.notes.lines().take(3).collect::<Vec<_>>().join(" / ");
        lines.push(Line::from(Span::styled(
            snippet,
            Style::default().fg(theme::TEXT_SECONDARY),
        )));
    }
    lines
}

fn centered(text: &str, color: ratatui::style::Color) -> Line<'static> {
    Line::from(Span::styled(text.to_string(), Style::default().fg(color)))
}
