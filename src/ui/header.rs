use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::data::process::AiState;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    // Left: logo + version
    let logo = Line::from(vec![
        Span::styled("Dofek", Style::default().fg(theme::ACCENT_INDIGO).add_modifier(Modifier::BOLD)),
        Span::styled(" v0.1", Style::default().fg(theme::TEXT_DIM)),
    ]);
    f.render_widget(Paragraph::new(logo), chunks[0]);

    // Center: AI workload badge
    let ai_procs: Vec<_> = app.data.processes.iter()
        .filter(|p| p.is_ai_workload && p.ai_state != AiState::None)
        .collect();

    let center = if let Some(proc) = ai_procs.first() {
        let (dot, state_color) = match proc.ai_state {
            AiState::Inferring => ("●", theme::ACCENT_PURPLE),
            AiState::Loading => ("●", theme::ACCENT_AMBER),
            AiState::Idle => ("○", theme::TEXT_DIM),
            AiState::None => ("", theme::TEXT_DIM),
        };
        Line::from(vec![
            Span::styled(dot, Style::default().fg(state_color)),
            Span::raw(" "),
            Span::styled(&proc.name, Style::default().fg(theme::TEXT_PRIMARY)),
            Span::raw(" "),
            Span::styled(proc.ai_state.to_string(), Style::default().fg(state_color)),
        ])
    } else {
        Line::from(Span::styled("no ai workloads", Style::default().fg(theme::TEXT_DIM)))
    };
    f.render_widget(Paragraph::new(center).alignment(Alignment::Center), chunks[1]);

    // Right: hostname + uptime
    let hostname = app.data.hostname.as_str();
    let right = Line::from(vec![
        Span::styled(hostname, Style::default().fg(theme::TEXT_SECONDARY)),
    ]);
    f.render_widget(Paragraph::new(right).alignment(Alignment::Right), chunks[2]);
}
