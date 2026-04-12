use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Row, Table};
use ratatui::Frame;
use ratatui::layout::Constraint;

use crate::app::App;
use crate::data::process::AiState;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" PROCESSES ", Style::default().fg(theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("sort: {} ", app.sort_column.label()),
                Style::default().fg(theme::TEXT_DIM),
            ),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_PANEL));

    let show_vram = app.data.nvml_available || app.data.processes.iter().any(|p| p.vram_bytes.is_some());

    // Header row
    let header_cells = if show_vram {
        vec!["NAME", "PID", "CPU%", "MEM", "VRAM", "AI"]
    } else {
        vec!["NAME", "PID", "CPU%", "MEM", "AI"]
    };

    let header = Row::new(header_cells.iter().map(|h| {
        Span::styled(*h, Style::default().fg(theme::TEXT_SECONDARY).add_modifier(Modifier::BOLD))
    }));

    let max_procs = app.config.display.process_count;
    let rows: Vec<Row> = app.data.processes.iter()
        .take(max_procs)
        .map(|p| {
            let mem_str = format_bytes(p.memory_bytes);
            let vram_str = p.vram_bytes.map(|v| format_bytes(v)).unwrap_or_else(|| "—".to_string());

            let ai_span = match p.ai_state {
                AiState::Inferring => Span::styled("● infer", Style::default().fg(theme::ACCENT_PURPLE)),
                AiState::Loading => Span::styled("● load", Style::default().fg(theme::ACCENT_AMBER)),
                AiState::Idle => Span::styled("○ idle", Style::default().fg(theme::TEXT_DIM)),
                AiState::None => Span::raw(""),
            };

            let name_style = if p.is_ai_workload {
                Style::default().fg(theme::AI_BADGE)
            } else {
                Style::default().fg(theme::TEXT_PRIMARY)
            };

            let mut cells = vec![
                Span::styled(truncate(&p.name, 20), name_style),
                Span::styled(format!("{:>6}", p.pid), Style::default().fg(theme::TEXT_DIM)),
                Span::styled(format!("{:>5.1}", p.cpu_percent), Style::default().fg(theme::TEXT_PRIMARY)),
                Span::styled(mem_str, Style::default().fg(theme::MEM_COLOR)),
            ];
            if show_vram {
                cells.push(Span::styled(vram_str, Style::default().fg(theme::VRAM_COLOR)));
            }
            cells.push(ai_span);

            Row::new(cells)
        })
        .collect();

    let widths = if show_vram {
        vec![
            Constraint::Length(22),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Min(8),
        ]
    } else {
        vec![
            Constraint::Length(22),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Min(8),
        ]
    };

    let table = Table::new(rows, &widths)
        .header(header)
        .block(block);

    f.render_widget(table, area);
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
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
