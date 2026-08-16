use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{App, CategoryFilter};
use crate::data::process::{AiState, ProcessCategory};
use crate::plugin::PluginState;
use crate::plugin::protocol;
use crate::ui::text::truncate_with;
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

    // The dock renders every panel and every entry a plugin reports (they are
    // already bounded at ingest by `plugin::sanitize`), so its height is driven
    // by content rather than by a hard cap. What bounds it is the *budget*: a
    // third of the watchlist when collapsed, everything the process table
    // doesn't need when expanded with `P`. Overflow becomes a "+N more" line
    // rather than silently dropped panels.
    let dock_lines = plugin_dock_lines(app);
    let dock_budget = if app.plugin_dock_expanded {
        inner.height.saturating_sub(MIN_TABLE_ROWS + 2) // header + tabs + table
    } else {
        inner.height / 3
    }
    .max(2); // border + at least one content line
    let plugin_dock_height = (1 + dock_lines.len() as u16).min(dock_budget); // 1 for border

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),           // PROCESSES header + sort buttons
            Constraint::Length(1),           // category filter tabs
            Constraint::Min(MIN_TABLE_ROWS), // process table
            Constraint::Length(plugin_dock_height), // plugin dock
        ])
        .split(inner);

    render_header(f, chunks[0], app);
    render_category_tabs(f, chunks[1], app);
    render_process_table(f, chunks[2], app);
    render_plugin_dock(f, chunks[3], app, dock_lines);
}

/// Rows the process table is never squeezed below, whatever the dock wants.
const MIN_TABLE_ROWS: u16 = 4;

/// Narrower than this and the name column carries the process name alone —
/// a two-character name plus a two-character label helps nobody.
const MIN_NAME_WITH_LABEL: usize = 14;

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

        // A plugin's label (an Ollama model name, a container name) is the
        // whole point of the annotation protocol, so it shares the name cell
        // rather than needing a column of its own. It is budgeted from the
        // cell — the process name keeps priority and the label is dropped
        // entirely when the column is too narrow to carry both.
        let name_budget = name_width.saturating_sub(2);
        let mut name_spans = vec![Span::raw(cat_icon)];
        match p.plugin_label.as_deref().filter(|l| !l.is_empty()) {
            Some(label) if name_budget >= MIN_NAME_WITH_LABEL => {
                let label_budget = (name_budget / 3).min(label.chars().count() + 1);
                name_spans.push(Span::styled(
                    truncate_with(&p.name, name_budget - label_budget, ".."),
                    name_style,
                ));
                name_spans.push(Span::styled(
                    format!(" {}", truncate_with(label, label_budget.saturating_sub(1), "…")),
                    Style::default().fg(theme::TEXT_DIM).add_modifier(Modifier::ITALIC),
                ));
            }
            _ => name_spans.push(Span::styled(
                truncate_with(&p.name, name_budget, ".."),
                name_style,
            )),
        }

        let mut cells: Vec<Cell> = vec![
            Cell::from(Line::from(name_spans)),
            Span::styled(format!("{:>5}", p.pid), Style::default().fg(theme::TEXT_DIM)).into(),
            Span::styled(format!("{:>4.1}", p.cpu_percent), Style::default().fg(cpu_color)).into(),
            Span::styled(format!("{:>5}", mem_str), Style::default().fg(theme::MEM_COLOR)).into(),
        ];

        if show_vram {
            let vram_color = if p.vram_bytes.is_some() { theme::GPU_COLOR } else { theme::TEXT_DIM };
            cells.push(Span::styled(format!("{:>5}", vram_str), Style::default().fg(vram_color)).into());
        }

        cells.push(ai_span.into());

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

/// Style lookup for a `PanelEntry.style` string. Unknown styles fall back to
/// the normal colour rather than being rejected — the protocol is open, and a
/// plugin from a newer schema shouldn't render as an error.
fn entry_style(style: &str) -> Style {
    match style {
        "accent" => Style::default().fg(theme::CPU_COLOR),
        "dim" => Style::default().fg(theme::TEXT_DIM),
        "warn" => Style::default().fg(theme::WARN_COLOR),
        "error" => Style::default().fg(theme::CRIT_COLOR),
        _ => Style::default().fg(theme::TEXT_SECONDARY),
    }
}

/// Build every line the plugin dock would like to draw, unclamped.
///
/// The first panel's entries ride on the plugin's own line — that keeps the
/// common single-panel plugin at one row, the density the dock had before —
/// and each additional panel gets an indented line of its own. The caller
/// decides how many of these actually fit.
fn plugin_dock_lines(app: &App) -> Vec<Line<'_>> {
    if app.data.plugin_statuses.is_empty() {
        return vec![Line::from(Span::styled(
            "No plugins connected",
            Style::default().fg(theme::TEXT_DIM),
        ))];
    }

    let mut lines = Vec::new();

    for status in &app.data.plugin_statuses {
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

        let panels = status.response.as_ref().map(|r| r.panels.as_slice()).unwrap_or(&[]);

        if let Some(first) = panels.first() {
            push_entry_spans(&mut spans, first);
        }

        // A non-healthy state is the one thing that must never be pushed off
        // the line by panel content, so it goes on the plugin's own row.
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
            PluginState::Healthy => {}
        }

        lines.push(Line::from(spans));

        for panel in panels.iter().skip(1) {
            let mut panel_spans = vec![
                Span::raw("  "),
                Span::styled(
                    panel.label.to_lowercase(),
                    Style::default().fg(theme::TEXT_DIM).add_modifier(Modifier::ITALIC),
                ),
            ];
            push_entry_spans(&mut panel_spans, panel);
            lines.push(Line::from(panel_spans));
        }
    }

    lines
}

/// Append a panel's entries as `key value` pairs. Every entry is rendered —
/// `sanitize` already caps a panel at 16 of them, and horizontal overflow is
/// clipped by the `Paragraph` rather than costing a row.
fn push_entry_spans<'a>(spans: &mut Vec<Span<'a>>, panel: &'a protocol::Panel) {
    for entry in &panel.content {
        spans.push(Span::raw("  "));
        if !entry.key.is_empty() {
            spans.push(Span::styled(
                format!("{} ", entry.key),
                Style::default().fg(theme::TEXT_DIM),
            ));
        }
        spans.push(Span::styled(&entry.value, entry_style(&entry.style)));
    }
}

fn render_plugin_dock(f: &mut Frame, area: Rect, app: &App, mut lines: Vec<Line<'_>>) {
    let title = if app.plugin_dock_expanded {
        " PLUGINS ▾ "
    } else {
        " PLUGINS ▸ "
    };
    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(theme::TEXT_DIM)))
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme::BORDER2))
        .style(Style::default().bg(theme::BG_PRIMARY));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let capacity = inner.height as usize;
    if capacity == 0 {
        return;
    }

    // Trade the last visible row for an honest overflow marker rather than
    // truncating in silence — the dock is the only place a plugin's output is
    // visible, so "there is more" has to be discoverable.
    if lines.len() > capacity {
        let hidden = lines.len() - (capacity - 1);
        lines.truncate(capacity - 1);
        let hint = if app.plugin_dock_expanded {
            format!("  +{hidden} more")
        } else {
            format!("  +{hidden} more — P to expand")
        };
        lines.push(Line::from(Span::styled(hint, Style::default().fg(theme::TEXT_DIM))));
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::data::DataSnapshot;
    use crate::data::process::{AiState, ProcessCategory, ProcessInfo};
    use crate::plugin::{PluginState, PluginStatus};
    use crate::plugin::protocol::{Panel, PanelEntry, PollResponse};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    fn test_app() -> App {
        let config: Config = toml::from_str("").expect("empty config uses all defaults");
        App::new(
            config,
            crate::telemetry::TelemetryHandle::disabled(),
            Arc::new(AtomicU64::new(500)),
        )
    }

    fn proc(pid: u32, name: &str, label: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cpu_percent: 4.0,
            memory_bytes: 256 * 1024 * 1024,
            vram_bytes: None,
            is_ai_workload: false,
            ai_state: AiState::None,
            category: ProcessCategory::None,
            plugin_label: label.map(str::to_string),
        }
    }

    fn panel(label: &str, entries: &[(&str, &str)]) -> Panel {
        Panel {
            id: label.to_lowercase(),
            label: label.to_string(),
            content: entries
                .iter()
                .map(|(k, v)| PanelEntry {
                    key: k.to_string(),
                    value: v.to_string(),
                    style: "normal".into(),
                })
                .collect(),
        }
    }

    fn status(name: &str, state: PluginState, panels: Vec<Panel>) -> PluginStatus {
        PluginStatus {
            name: name.to_string(),
            display_name: name.to_string(),
            state,
            response: Some(PollResponse { panels, ..Default::default() }),
        }
    }

    /// Render just the watchlist at `(w, h)` and return the buffer as text.
    fn render_to_string(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn app_with(statuses: Vec<PluginStatus>, names: &[&str]) -> App {
        let mut app = test_app();
        app.update_data(DataSnapshot {
            processes: names.iter().enumerate().map(|(i, n)| proc(i as u32 + 1, n, None)).collect(),
            plugin_statuses: statuses,
            ..Default::default()
        });
        app
    }

    /// The v1.5 dock showed `panels.first()` and `.take(2)` of its entries;
    /// everything else a plugin reported was invisible. Ingest already bounds
    /// the payload, so the renderer has no business dropping it.
    #[test]
    fn dock_renders_every_panel_and_entry() {
        let mut app = app_with(
            vec![status(
                "ollama",
                PluginState::Healthy,
                vec![
                    panel("models", &[("loaded", "AAA"), ("queued", "BBB"), ("evicted", "CCC")]),
                    panel("gpu", &[("layers", "DDD")]),
                    panel("io", &[("tokens", "EEE")]),
                ],
            )],
            &["init"],
        );
        app.plugin_dock_expanded = true;

        let out = render_to_string(&app, 100, 40);
        for value in ["AAA", "BBB", "CCC", "DDD", "EEE"] {
            assert!(out.contains(value), "dock dropped entry {value}\n{out}");
        }
    }

    /// The dock is allowed to grow, but never at the process table's expense.
    #[test]
    fn collapsed_dock_never_starves_the_process_table() {
        let noisy: Vec<PluginStatus> = (0..6)
            .map(|i| {
                status(
                    &format!("plug{i}"),
                    PluginState::Healthy,
                    (0..8).map(|p| panel(&format!("panel{p}"), &[("k", "v")])).collect(),
                )
            })
            .collect();
        let app = app_with(noisy, &["alpha", "bravo", "charlie", "delta"]);

        let out = render_to_string(&app, 100, 30);
        // All four processes still have a row.
        for name in ["alpha", "bravo", "charlie", "delta"] {
            assert!(out.contains(name), "process {name} was pushed out by the dock\n{out}");
        }
    }

    /// Truncation has to be visible: the dock is the only place a plugin's
    /// output surfaces, so "there is more" must be discoverable.
    #[test]
    fn overflowing_dock_says_how_much_is_hidden() {
        let app = app_with(
            vec![status(
                "busy",
                PluginState::Healthy,
                (0..8).map(|p| panel(&format!("panel{p}"), &[("k", "v")])).collect(),
            )],
            &["init"],
        );

        // 18 rows of watchlist inner height ⇒ a collapsed budget of 6, so the
        // plugin's 8 lines cannot all fit.
        let out = render_to_string(&app, 100, 20);
        assert!(out.contains("more"), "no overflow marker in a truncated dock\n{out}");
        assert!(out.contains("P to expand"), "overflow marker omits the key hint\n{out}");
    }

    /// `P` has to actually buy the user something.
    #[test]
    fn expanding_the_dock_reveals_more_panels() {
        let panels: Vec<Panel> = (0..8)
            .map(|p| panel(&format!("panel{p}"), &[("k", &format!("value{p}"))]))
            .collect();
        let mut app = app_with(vec![status("busy", PluginState::Healthy, panels)], &["init"]);

        let collapsed = render_to_string(&app, 100, 20).matches("value").count();
        app.plugin_dock_expanded = true;
        let expanded = render_to_string(&app, 100, 20).matches("value").count();

        assert!(
            expanded > collapsed,
            "expanding showed no extra panels ({collapsed} → {expanded})"
        );
    }

    /// `plugin_label` was set by the annotation path since v0.3 and rendered
    /// nowhere. It is the payoff of the whole annotation protocol.
    #[test]
    fn plugin_label_is_rendered_next_to_the_process_name() {
        let mut app = test_app();
        app.update_data(DataSnapshot {
            processes: vec![proc(1, "ollama", Some("llama3"))],
            ..Default::default()
        });

        let out = render_to_string(&app, 100, 30);
        assert!(out.contains("ollama"), "process name missing\n{out}");
        assert!(out.contains("llama3"), "plugin label missing\n{out}");
    }

    /// When the column can't carry both, the process name wins — a label is
    /// supplementary, an unidentifiable row is not.
    #[test]
    fn narrow_columns_drop_the_label_not_the_name() {
        let mut app = test_app();
        app.update_data(DataSnapshot {
            processes: vec![proc(1, "ollama", Some("llama3"))],
            ..Default::default()
        });

        // Watchlist gets ~42% of the width; at 60 the name column is under
        // MIN_NAME_WITH_LABEL.
        let out = render_to_string(&app, 60, 20);
        assert!(out.contains("olla"), "process name was sacrificed for the label\n{out}");
    }

    /// Labels and panel values are plugin-controlled strings, so they reach
    /// the same char-boundary hazard the process names did.
    #[test]
    fn multibyte_plugin_text_does_not_panic_at_any_width() {
        let mut app = test_app();
        app.update_data(DataSnapshot {
            processes: vec![
                proc(1, "字幕処理サービス", Some("🔥モデル🔥")),
                proc(2, "ollama", Some("字幕処理サービス管理常駐")),
            ],
            plugin_statuses: vec![status(
                "字幕",
                PluginState::Unhealthy,
                vec![panel("パネル", &[("キー", "🔥🚀🎉🧠💾")])],
            )],
            ..Default::default()
        });

        for expanded in [false, true] {
            app.plugin_dock_expanded = expanded;
            for (w, h) in [(200u16, 60u16), (80, 24), (60, 20), (40, 15), (24, 10), (12, 8)] {
                let _ = render_to_string(&app, w, h);
            }
        }
    }

    /// A crashed plugin's state word must survive however much panel content
    /// is competing for the same line.
    #[test]
    fn unhealthy_state_is_always_on_the_plugin_line() {
        let app = app_with(
            vec![status(
                "flaky",
                PluginState::Crashed,
                vec![panel("p", &[("k", "v"); 12])],
            )],
            &["init"],
        );

        let lines = plugin_dock_lines(&app);
        let head = format!("{:?}", lines[0]);
        assert!(head.contains("crashed"), "state word left off the plugin line: {head}");
    }
}
