use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{App, CategoryFilter, ConfirmKill, ProcessRow};
use crate::data::process::{AiState, ProcessCategory};
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let filter_label = match app.category_filter {
        CategoryFilter::All => "ALL",
        CategoryFilter::Ai => "AI",
        CategoryFilter::Dev => "DEV",
        CategoryFilter::Watch => "WATCH",
    };

    let view_label = if app.grouped_view { "TREE" } else { "FLAT" };

    // Build title with filter + search info
    let mut title_spans = vec![
        Span::styled(" PROCESSES ", Style::default().fg(theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("[{}] ", filter_label),
            Style::default().fg(theme::CPU_COLOR),
        ),
        Span::styled(
            format!("{} ", view_label),
            Style::default().fg(if app.grouped_view { theme::MEM_COLOR } else { theme::TEXT_DIM }),
        ),
        Span::styled(
            format!("sort: {} ", app.sort_column.label()),
            Style::default().fg(theme::TEXT_DIM),
        ),
    ];
    if !app.search_query.is_empty() && !app.search_active {
        title_spans.push(Span::styled(
            format!(" /{} ", app.search_query),
            Style::default().fg(theme::WARN_COLOR),
        ));
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_PANEL));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 3 || inner.width < 30 {
        return;
    }

    let show_vram = app.data.nvml_available || app.data.processes.iter().any(|p| p.vram_bytes.is_some());

    // Compute available width for name column
    let fixed_cols: u16 = if show_vram { 46 } else { 36 };
    let name_width = inner.width.saturating_sub(fixed_cols).max(16) as usize;

    // Reserve lines: 1 header, 1 bottom hint, 1 search bar (if active)
    let search_bar_height = if app.search_active { 1usize } else { 0 };
    let max_visible = (inner.height as usize).saturating_sub(2 + search_bar_height);

    // Header row
    let header_style = Style::default().fg(theme::TEXT_SECONDARY).add_modifier(Modifier::BOLD);
    let header_cells: Vec<Span> = if show_vram {
        vec![
            Span::styled("  NAME", header_style),
            Span::styled(format!("{:>7}", "PID"), header_style),
            Span::styled(format!("{:>7}", "CPU%"), header_style),
            Span::styled(format!("{:>9}", "MEM"), header_style),
            Span::styled(format!("{:>9}", "VRAM"), header_style),
            Span::styled("  AI", header_style),
        ]
    } else {
        vec![
            Span::styled("  NAME", header_style),
            Span::styled(format!("{:>7}", "PID"), header_style),
            Span::styled(format!("{:>7}", "CPU%"), header_style),
            Span::styled(format!("{:>9}", "MEM"), header_style),
            Span::styled("  AI", header_style),
        ]
    };
    let header = Row::new(header_cells);

    // Search bar
    if app.search_active {
        let search_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let cursor = if (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() / 500).is_multiple_of(2)
        { "█" } else { " " };
        let total = app.filtered_processes().len();
        let search_line = Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme::WARN_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(app.search_query.as_str(), Style::default().fg(theme::TEXT_PRIMARY)),
            Span::styled(cursor, Style::default().fg(theme::WARN_COLOR)),
            Span::styled(
                format!("  ({total} match{})", if total == 1 { "" } else { "es" }),
                Style::default().fg(theme::TEXT_DIM),
            ),
        ]);
        f.render_widget(Paragraph::new(search_line), search_area);
    }

    let table_y = inner.y + search_bar_height as u16;
    let table_h = inner.height.saturating_sub(1 + search_bar_height as u16);

    // Build rows — grouped or flat
    let rows: Vec<Row>;
    let total: usize;

    if app.grouped_view {
        let grouped = app.grouped_rows();
        total = grouped.len();
        let selected = app.selected_process.unwrap_or(0).min(total.saturating_sub(1));
        let scroll = compute_scroll(app.process_scroll, selected, max_visible, total);

        rows = grouped.iter()
            .enumerate()
            .skip(scroll)
            .take(max_visible)
            .map(|(i, row)| render_grouped_row(row, i, app, name_width, show_vram))
            .collect();
    } else {
        let filtered = app.filtered_processes();
        total = filtered.len();
        let selected = app.selected_process.unwrap_or(0).min(total.saturating_sub(1));
        let scroll = compute_scroll(app.process_scroll, selected, max_visible, total);

        rows = filtered.iter()
            .enumerate()
            .skip(scroll)
            .take(max_visible)
            .map(|(i, p)| render_flat_row(p, i, app, name_width, show_vram))
            .collect();
    }

    if total == 0 {
        let table_area = Rect::new(inner.x, table_y, inner.width, table_h);
        let msg = if app.data.processes.is_empty() {
            "Waiting for data..."
        } else if !app.search_query.is_empty() {
            "No processes match search"
        } else {
            "No matching processes"
        };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(theme::TEXT_DIM))),
            table_area,
        );
    } else {
        let widths = if show_vram {
            vec![
                Constraint::Min(20),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
            ]
        } else {
            vec![
                Constraint::Min(20),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(10),
            ]
        };

        let table_area = Rect::new(inner.x, table_y, inner.width, table_h);
        let table = Table::new(rows, &widths).header(header);
        f.render_widget(table, table_area);
    }

    // Bottom hint bar
    let hint_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);

    if let Some(ref status) = app.kill_status {
        let color = if status.starts_with("Failed") || status.contains("failed") {
            theme::CRIT_COLOR
        } else {
            theme::GREEN_COLOR
        };
        f.render_widget(
            Paragraph::new(Span::styled(status.as_str(), Style::default().fg(color))),
            hint_area,
        );
    } else if app.search_active {
        let hints = Line::from(vec![
            Span::styled(" type ", Style::default().fg(theme::WARN_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("to search  ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled("enter ", Style::default().fg(theme::WARN_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("confirm  ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled("esc ", Style::default().fg(theme::WARN_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("clear  ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled("del ", Style::default().fg(theme::WARN_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("kill", Style::default().fg(theme::TEXT_DIM)),
        ]);
        f.render_widget(Paragraph::new(hints), hint_area);
    } else {
        let mut hint_spans = vec![
            Span::styled(" / ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("search  ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled("↑↓ ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("nav  ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled("del/x ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("kill  ", Style::default().fg(theme::TEXT_DIM)),
        ];
        if !app.search_query.is_empty() || app.category_filter != CategoryFilter::All {
            hint_spans.push(Span::styled("X ", Style::default().fg(theme::CRIT_COLOR).add_modifier(Modifier::BOLD)));
            hint_spans.push(Span::styled("kill all  ", Style::default().fg(theme::TEXT_DIM)));
        }
        hint_spans.extend([
            Span::styled("t ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(if app.grouped_view { "flat  " } else { "tree  " }, Style::default().fg(theme::TEXT_DIM)),
        ]);
        if app.grouped_view {
            hint_spans.extend([
                Span::styled("→← ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
                Span::styled("expand  ", Style::default().fg(theme::TEXT_DIM)),
            ]);
        }
        hint_spans.extend([
            Span::styled("tab ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("sort  ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled("esc ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("back", Style::default().fg(theme::TEXT_DIM)),
        ]);
        f.render_widget(Paragraph::new(Line::from(hint_spans)), hint_area);
    }

    // Kill confirmation overlay
    if let Some(ref ck) = app.confirm_kill {
        render_kill_confirm(f, area, ck);
    }
}

fn render_flat_row(p: &crate::data::process::ProcessInfo, i: usize, app: &App, name_width: usize, show_vram: bool) -> Row<'static> {
    let is_selected = app.selected_process == Some(i);

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

    let selector = if is_selected { "▸ " } else { "  " };
    let max_name = name_width.saturating_sub(2);

    let mut cells = vec![
        Span::styled(format!("{}{}", selector, truncate(&p.name, max_name)), name_style),
        Span::styled(format!("{:>7}", p.pid), Style::default().fg(theme::TEXT_DIM)),
        Span::styled(format!("{:>7.1}", p.cpu_percent), Style::default().fg(theme::TEXT_PRIMARY)),
        Span::styled(format!("{:>9}", format_bytes(p.memory_bytes)), Style::default().fg(theme::MEM_COLOR)),
    ];
    if show_vram {
        let vram_str = p.vram_bytes.map(format_bytes).unwrap_or_else(|| "—".to_string());
        cells.push(Span::styled(format!("{:>9}", vram_str), Style::default().fg(theme::VRAM_COLOR)));
    }
    cells.push(ai_span);

    let row = Row::new(cells);
    if is_selected {
        row.style(Style::default().bg(theme::BG_SURFACE2))
    } else {
        row
    }
}

fn render_grouped_row(row: &ProcessRow<'_>, i: usize, app: &App, name_width: usize, show_vram: bool) -> Row<'static> {
    let is_selected = app.selected_process == Some(i);

    match row {
        ProcessRow::Group { name, count, cpu_total, mem_total, vram_total, expanded, category, .. } => {
            let arrow = if *expanded { "▾ " } else { "▸ " };
            let selector = if is_selected { arrow } else { if *expanded { "▾ " } else { "  " } };

            let cat_color = match category {
                ProcessCategory::Ai => theme::AI_COLOR,
                ProcessCategory::Dev => theme::DEV_COLOR,
                ProcessCategory::Watch => theme::WATCH_COLOR,
                ProcessCategory::None => theme::TEXT_PRIMARY,
            };

            let max_name = name_width.saturating_sub(2);
            let display = format!("{} ({})", name, count);

            let mut cells = vec![
                Span::styled(format!("{}{}", selector, truncate(&display, max_name)), Style::default().fg(cat_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:>7}", ""), Style::default().fg(theme::TEXT_DIM)),
                Span::styled(format!("{:>7.1}", cpu_total), Style::default().fg(theme::TEXT_PRIMARY)),
                Span::styled(format!("{:>9}", format_bytes(*mem_total)), Style::default().fg(theme::MEM_COLOR)),
            ];
            if show_vram {
                let vram_str = if *vram_total > 0 { format_bytes(*vram_total) } else { "—".to_string() };
                cells.push(Span::styled(format!("{:>9}", vram_str), Style::default().fg(theme::VRAM_COLOR)));
            }
            cells.push(Span::raw(""));

            let row = Row::new(cells);
            if is_selected {
                row.style(Style::default().bg(theme::BG_SURFACE2))
            } else {
                row
            }
        }
        ProcessRow::Process(p) => {
            let ai_span = match p.ai_state {
                AiState::Inferring => Span::styled("● infer", Style::default().fg(theme::ACCENT_PURPLE)),
                AiState::Loading => Span::styled("● load", Style::default().fg(theme::ACCENT_AMBER)),
                AiState::Idle => Span::styled("○ idle", Style::default().fg(theme::TEXT_DIM)),
                AiState::None => Span::raw(""),
            };

            // Indent child processes
            let selector = if is_selected { "  ▸ " } else { "    " };
            let max_name = name_width.saturating_sub(4);

            let name_style = Style::default().fg(theme::TEXT_SECONDARY);

            let mut cells = vec![
                Span::styled(format!("{}{}", selector, truncate(&p.name, max_name)), name_style),
                Span::styled(format!("{:>7}", p.pid), Style::default().fg(theme::TEXT_DIM)),
                Span::styled(format!("{:>7.1}", p.cpu_percent), Style::default().fg(theme::TEXT_PRIMARY)),
                Span::styled(format!("{:>9}", format_bytes(p.memory_bytes)), Style::default().fg(theme::MEM_COLOR)),
            ];
            if show_vram {
                let vram_str = p.vram_bytes.map(format_bytes).unwrap_or_else(|| "—".to_string());
                cells.push(Span::styled(format!("{:>9}", vram_str), Style::default().fg(theme::VRAM_COLOR)));
            }
            cells.push(ai_span);

            let row = Row::new(cells);
            if is_selected {
                row.style(Style::default().bg(theme::BG_SURFACE2))
            } else {
                row
            }
        }
    }
}

fn render_kill_confirm(f: &mut Frame, area: Rect, ck: &ConfirmKill) {
    let msg = match ck {
        ConfirmKill::Single { pid, name } => format!(" Kill {} (PID {})? ", name, pid),
        ConfirmKill::Batch { targets } => {
            let count = targets.len();
            let mut names: Vec<&str> = targets.iter().map(|(_, n)| n.as_str()).collect();
            names.dedup();
            let preview: String = names.iter().take(3).copied().collect::<Vec<_>>().join(", ");
            let suffix = if names.len() > 3 { ", ..." } else { "" };
            format!(" Kill {count} processes ({preview}{suffix})? ")
        }
    };
    let width = (msg.len() as u16 + 6).min(area.width.saturating_sub(4));
    let height = 5u16;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup);

    let title = match ck {
        ConfirmKill::Single { .. } => " CONFIRM KILL ",
        ConfirmKill::Batch { .. } => " CONFIRM KILL ALL ",
    };

    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(theme::CRIT_COLOR).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::CRIT_COLOR))
        .style(Style::default().bg(theme::BG_PANEL));

    let lines = vec![
        Line::from(Span::styled(msg, Style::default().fg(theme::TEXT_PRIMARY))),
        Line::from(vec![
            Span::styled("  y ", Style::default().fg(theme::CRIT_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("yes    ", Style::default().fg(theme::TEXT_SECONDARY)),
            Span::styled("any ", Style::default().fg(theme::TEXT_DIM).add_modifier(Modifier::BOLD)),
            Span::styled("cancel", Style::default().fg(theme::TEXT_SECONDARY)),
        ]),
    ];

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, popup);
}

fn compute_scroll(current_scroll: usize, selected: usize, visible: usize, total: usize) -> usize {
    if total <= visible {
        return 0;
    }
    let mut scroll = current_scroll;
    if selected < scroll {
        scroll = selected;
    } else if selected >= scroll + visible {
        scroll = selected - visible + 1;
    }
    scroll.min(total - visible)
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
