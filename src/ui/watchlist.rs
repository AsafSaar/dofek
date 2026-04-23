use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{App, CategoryFilter};
use crate::data::process::{AiState, ProcessCategory};
use crate::plugin::PluginState;
use crate::ui::theme;

/// Render the process watchlist panel with category tabs and plugin dock.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_SURFACE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 6 || inner.width < 25 {
        return;
    }

    // Dock height: 2 (border + empty message) if no plugins,
    // 2 + number of plugin lines (1 per plugin) if plugins present, capped at 6.
    let plugin_line_count = if app.data.plugin_statuses.is_empty() {
        1u16 // "No plugins connected"
    } else {
        app.data.plugin_statuses.len() as u16
    };
    let plugin_dock_height = (1 + plugin_line_count).min(6); // 1 for border

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),           // PROCESSES header + sort buttons
            Constraint::Length(1),           // category filter tabs
            Constraint::Min(4),             // process table
            Constraint::Length(plugin_dock_height), // plugin dock
        ])
        .split(inner);

    render_header(f, chunks[0], app);
    render_category_tabs(f, chunks[1], app);
    render_process_table(f, chunks[2], app);
    render_plugin_dock(f, chunks[3], app);
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let sort_cols = [
        ("CPU", crate::app::SortColumn::Cpu),
        ("MEM", crate::app::SortColumn::Memory),
        ("VRAM", crate::app::SortColumn::Vram),
    ];

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(
        " PROCESSES ",
        Style::default().fg(theme::TEXT_SECONDARY).add_modifier(Modifier::BOLD),
    ));

    // Push right: sort buttons
    // Calculate padding to push sort buttons to the right
    let sort_width: usize = sort_cols.iter().map(|(l, _)| l.len() + 2).sum::<usize>() + 1;
    let label_width = 12; // " PROCESSES "
    let padding = area.width as usize - label_width - sort_width;
    if area.width as usize > label_width + sort_width {
        spans.push(Span::raw(" ".repeat(padding)));
    }

    for (label, col) in &sort_cols {
        if *col == app.sort_column {
            spans.push(Span::styled(
                format!("[{label}]"),
                Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {label} "),
                Style::default().fg(theme::TEXT_DIM),
            ));
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_category_tabs(f: &mut Frame, area: Rect, app: &App) {
    let tabs = [
        (CategoryFilter::All, "ALL", theme::CPU_COLOR),
        (CategoryFilter::Ai, "● AI", theme::AI_COLOR),
        (CategoryFilter::Dev, "■ DEV", theme::DEV_COLOR),
        (CategoryFilter::Watch, "★ WATCH", theme::WATCH_COLOR),
    ];

    let mut spans: Vec<Span> = Vec::new();
    for (filter, label, color) in &tabs {
        if *filter == app.category_filter {
            spans.push(Span::styled(
                format!(" {label} "),
                Style::default().fg(*color).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {label} "),
                Style::default().fg(theme::TEXT_DIM),
            ));
        }
    }

    // Sort indicator on the right
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!("sort:{}", app.sort_column.label()),
        Style::default().fg(theme::TEXT_DIM),
    ));

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_process_table(f: &mut Frame, area: Rect, app: &App) {
    let show_vram = app.data.nvml_available || app.data.processes.iter().any(|p| p.vram_bytes.is_some());

    // Compute available width for the name column:
    // Fixed columns: PID(6) + CPU%(5) + MEM(6) + STATUS(8) = 25, plus VRAM(6) if shown
    let fixed_cols: u16 = if show_vram { 31 } else { 25 };
    let name_width = (area.width.saturating_sub(fixed_cols)) as usize;

    // Header
    let header_cells = if show_vram {
        vec!["  NAME", "PID", "CPU%", "MEM", "VRAM", ""]
    } else {
        vec!["  NAME", "PID", "CPU%", "MEM", ""]
    };

    let header = Row::new(header_cells.iter().map(|h| {
        Span::styled(*h, Style::default().fg(theme::TEXT_DIM))
    }));

    // Show as many processes as fit in the available height (minus 1 for header)
    let max_visible = (area.height as usize).saturating_sub(1);

    // Filter and build rows
    let filtered: Vec<_> = app.data.processes.iter()
        .filter(|p| match app.category_filter {
            CategoryFilter::All => true,
            CategoryFilter::Ai => p.category == ProcessCategory::Ai,
            CategoryFilter::Dev => p.category == ProcessCategory::Dev,
            CategoryFilter::Watch => p.category == ProcessCategory::Watch,
        })
        .take(max_visible)
        .collect();

    if filtered.is_empty() {
        let msg = if app.data.processes.is_empty() {
            "Waiting for data...".to_string()
        } else {
            "No matching processes".to_string()
        };
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(theme::TEXT_DIM)),
            area,
        );
        return;
    }

    let rows: Vec<Row> = filtered.iter().map(|p| {
        let (cat_icon, _cat_color) = match p.category {
            ProcessCategory::Ai => ("● ", theme::AI_COLOR),
            ProcessCategory::Dev => ("■ ", theme::DEV_COLOR),
            ProcessCategory::Watch => ("★ ", theme::WATCH_COLOR),
            ProcessCategory::None => ("  ", theme::TEXT_DIM),
        };

        let name_style = match p.category {
            ProcessCategory::Ai => Style::default().fg(theme::AI_COLOR),
            ProcessCategory::Dev => Style::default().fg(theme::DEV_COLOR),
            ProcessCategory::Watch => Style::default().fg(theme::WATCH_COLOR),
            ProcessCategory::None => Style::default().fg(theme::TEXT_PRIMARY),
        };

        let cpu_color = if p.cpu_percent > 20.0 {
            theme::CRIT_COLOR
        } else if p.cpu_percent > 12.0 {
            theme::WARN_COLOR
        } else {
            theme::TEXT_SECONDARY
        };

        let ai_span = match p.ai_state {
            AiState::Inferring => Span::styled("● infer", Style::default().fg(theme::AI_COLOR)),
            AiState::Loading => Span::styled("● load", Style::default().fg(theme::WARN_COLOR)),
            AiState::Idle => Span::styled("○ idle", Style::default().fg(theme::TEXT_DIM)),
            AiState::None => Span::raw(""),
        };

        let mem_str = format_bytes(p.memory_bytes);
        let vram_str = p.vram_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "—".to_string());

        let mut cells = vec![
            Span::styled(
                format!("{}{}", cat_icon, truncate(&p.name, name_width.saturating_sub(2))),
                name_style,
            ),
            Span::styled(format!("{:>5}", p.pid), Style::default().fg(theme::TEXT_DIM)),
            Span::styled(format!("{:>4.1}", p.cpu_percent), Style::default().fg(cpu_color)),
            Span::styled(format!("{:>5}", mem_str), Style::default().fg(theme::MEM_COLOR)),
        ];

        if show_vram {
            let vram_color = if p.vram_bytes.is_some() { theme::GPU_COLOR } else { theme::TEXT_DIM };
            cells.push(Span::styled(format!("{:>5}", vram_str), Style::default().fg(vram_color)));
        }

        cells.push(ai_span);

        let row_style = match p.category {
            ProcessCategory::Ai => Style::default().bg(theme::BG_SURFACE2),
            ProcessCategory::Dev => Style::default().bg(theme::BG_SURFACE),
            ProcessCategory::Watch => Style::default().bg(theme::BG_SURFACE2),
            ProcessCategory::None => Style::default(),
        };

        Row::new(cells).style(row_style)
    }).collect();

    let widths = if show_vram {
        vec![
            Constraint::Min(14),       // NAME (with category icon)
            Constraint::Length(6),      // PID
            Constraint::Length(5),      // CPU%
            Constraint::Length(6),      // MEM
            Constraint::Length(6),      // VRAM
            Constraint::Length(8),      // STATUS
        ]
    } else {
        vec![
            Constraint::Min(14),
            Constraint::Length(6),
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Length(8),
        ]
    };

    let table = Table::new(rows, &widths).header(header);
    f.render_widget(table, area);
}

fn render_plugin_dock(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Span::styled(" PLUGINS ", Style::default().fg(theme::TEXT_DIM)))
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme::BORDER2))
        .style(Style::default().bg(theme::BG_PRIMARY));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.data.plugin_statuses.is_empty() {
        let msg = Paragraph::new(Line::from(vec![
            Span::styled("No plugins connected", Style::default().fg(theme::TEXT_DIM)),
        ]));
        f.render_widget(msg, inner);
        return;
    }

    let lines: Vec<Line> = app
        .data
        .plugin_statuses
        .iter()
        .take(inner.height as usize)
        .map(|status| {
            let (dot, dot_color) = match status.state {
                PluginState::Healthy => ("●", theme::GREEN_COLOR),
                PluginState::Starting => ("○", theme::TEXT_DIM),
                PluginState::Unhealthy => ("●", theme::WARN_COLOR),
                PluginState::Crashed => ("●", theme::CRIT_COLOR),
            };

            let mut spans = vec![
                Span::styled(format!("{dot} "), Style::default().fg(dot_color)),
                Span::styled(
                    status.display_name.to_uppercase(),
                    Style::default().fg(theme::TEXT_SECONDARY).add_modifier(Modifier::BOLD),
                ),
            ];

            // Show first panel's content inline if available
            if let Some(ref response) = status.response
                && let Some(panel) = response.panels.first() {
                    for entry in panel.content.iter().take(2) {
                        let style = match entry.style.as_str() {
                            "accent" => Style::default().fg(theme::CPU_COLOR),
                            "dim" => Style::default().fg(theme::TEXT_DIM),
                            "warn" => Style::default().fg(theme::WARN_COLOR),
                            "error" => Style::default().fg(theme::CRIT_COLOR),
                            _ => Style::default().fg(theme::TEXT_SECONDARY),
                        };
                        spans.push(Span::raw("  "));
                        spans.push(Span::styled(&entry.value, style));
                    }
                }

            // Show state label for non-healthy states
            match status.state {
                PluginState::Crashed => {
                    spans.push(Span::styled("  crashed", Style::default().fg(theme::CRIT_COLOR)));
                }
                PluginState::Unhealthy => {
                    spans.push(Span::styled("  unhealthy", Style::default().fg(theme::WARN_COLOR)));
                }
                PluginState::Starting => {
                    spans.push(Span::styled("  starting...", Style::default().fg(theme::TEXT_DIM)));
                }
                _ => {}
            }

            Line::from(spans)
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1}G", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.0}M", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0}K", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 2 {
        format!("{}..", &s[..max_len - 2])
    } else {
        s[..max_len].to_string()
    }
}
