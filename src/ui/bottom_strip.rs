use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline, Widget};
use ratatui::Frame;

use crate::app::App;
use crate::ui::theme;

/// Render the bottom compact strip: CPU cores | GPU | MEM | NET.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(22),
            Constraint::Percentage(23),
        ])
        .split(area);

    render_cpu_compact(f, panels[0], app);
    render_gpu_compact(f, panels[1], app);
    render_mem_compact(f, panels[2], app);
    render_net_compact(f, panels[3], app);
}

fn render_cpu_compact(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" CPU ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(
                truncate(&app.data.cpu.name, area.width.saturating_sub(10) as usize),
                Style::default().fg(theme::TEXT_DIM),
            ),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_SURFACE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    // Supplemental sensor rows (CPU temp/power from LHM)
    let has_temp = app.data.cpu.temperature.is_some();
    let has_power = app.data.cpu.power.is_some();
    let sensor_rows = (has_temp as u16) + (has_power as u16);

    // Grid layout: cores in N columns x M rows, then optional sensors, then sparkline
    let cores = &app.data.cpu.per_core_load;
    let num_cols = 4u16.min(inner.width / 8); // 4 columns, each needs ~8 chars
    if num_cols == 0 || cores.is_empty() {
        f.render_widget(
            Paragraph::new("Waiting for data...").style(Style::default().fg(theme::TEXT_DIM)),
            inner,
        );
        return;
    }
    let num_rows = (cores.len() as u16).div_ceil(num_cols).min(inner.height.saturating_sub(2 + sensor_rows));
    let sparkline_height = 1u16.min(inner.height.saturating_sub(num_rows + sensor_rows));

    let mut constraints = vec![Constraint::Length(num_rows)];
    if has_temp { constraints.push(Constraint::Length(1)); }
    if has_power { constraints.push(Constraint::Length(1)); }
    constraints.push(Constraint::Length(sparkline_height));

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // Grid of core cells
    let col_width = inner.width / num_cols;
    for (i, &pct) in cores.iter().enumerate() {
        let col = (i as u16) % num_cols;
        let row = (i as u16) / num_cols;
        if row >= num_rows {
            break;
        }

        let x = vert[0].x + col * col_width;
        let y = vert[0].y + row;
        let w = if col == num_cols - 1 { inner.width - col * col_width } else { col_width };
        let cell_area = Rect::new(x, y, w, 1);

        let color = if pct > 90.0 { theme::CRIT_COLOR } else if pct > 75.0 { theme::WARN_COLOR } else { theme::CPU_COLOR };

        // Format: "C0▓▓▓░░ 23.4%"
        let label = format!("C{i}");
        let val = format!("{pct:.1}%");
        let bar_w = w.saturating_sub(label.len() as u16 + val.len() as u16 + 1);

        if bar_w < 2 {
            // Not enough space, just show label + value
            let text = format!("C{i} {pct:.0}%");
            f.render_widget(Paragraph::new(text).style(Style::default().fg(color)), cell_area);
            continue;
        }

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(label.len() as u16),
                Constraint::Length(bar_w),
                Constraint::Length(val.len() as u16 + 1),
            ])
            .split(cell_area);

        f.render_widget(
            Paragraph::new(label).style(Style::default().fg(theme::TEXT_DIM)),
            cols[0],
        );
        f.render_widget(
            ColorBar::new((pct as f64 / 100.0).clamp(0.0, 1.0), color),
            cols[1],
        );
        f.render_widget(
            Paragraph::new(format!(" {val}")).style(Style::default().fg(theme::TEXT_SECONDARY)),
            cols[2],
        );
    }

    // Temp and power rows (from LHM)
    let mut next_row = 1;
    if let Some(temp) = app.data.cpu.temperature {
        render_metric_bar(f, vert[next_row], "Temp", temp, 105.0, "°C", theme::WARN_COLOR);
        next_row += 1;
    }
    if let Some(power) = app.data.cpu.power {
        render_metric_bar(f, vert[next_row], "Powr", power, 200.0, "W", theme::NET_TX_COLOR);
        next_row += 1;
    }

    // Sparkline at bottom
    let spark_data = app.history.cpu_total.as_slice();
    f.render_widget(
        Sparkline::default().data(spark_data).max(100).style(Style::default().fg(theme::CPU_COLOR)),
        vert[next_row],
    );
}

fn render_gpu_compact(f: &mut Frame, area: Rect, app: &App) {
    let gpu = app.primary_gpu();
    let title_detail = gpu.map(|g| g.name.clone()).unwrap_or_else(|| "No GPU".to_string());

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" GPU ", Style::default().fg(theme::GPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(
                truncate(&title_detail, area.width.saturating_sub(10) as usize),
                Style::default().fg(theme::TEXT_DIM),
            ),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_SURFACE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(gpu) = gpu else {
        f.render_widget(
            Paragraph::new("No GPU detected").style(Style::default().fg(theme::TEXT_DIM)),
            inner,
        );
        return;
    };

    if inner.height < 4 {
        return;
    }

    let sparkline_height = 1u16.min(inner.height.saturating_sub(4));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // util
            Constraint::Length(1), // vram
            Constraint::Length(1), // temp
            Constraint::Length(1), // power
            Constraint::Min(sparkline_height),
        ])
        .split(inner);

    render_metric_bar(f, chunks[0], "Util", gpu.utilization, 100.0, "%", theme::GPU_COLOR);

    // VRAM: show GB value, bar based on percentage
    let vram_pct = if gpu.vram_total_mb > 0.0 { gpu.vram_used_mb / gpu.vram_total_mb * 100.0 } else { 0.0 };
    render_metric_bar_custom(f, chunks[1], "VRAM", vram_pct, &format!("{:.1} GB", gpu.vram_used_mb / 1024.0), theme::GPU_COLOR);

    render_metric_bar(f, chunks[2], "Temp", gpu.temperature, 100.0, "°C", theme::WARN_COLOR);
    render_metric_bar(f, chunks[3], "Powr", gpu.power_watts, 350.0, "W", theme::NET_TX_COLOR);

    let spark_data = app.history.gpu_util.as_slice();
    let max_val = spark_data.iter().copied().max().unwrap_or(1).max(1);
    f.render_widget(
        Sparkline::default().data(spark_data).max(max_val).style(Style::default().fg(theme::GPU_COLOR)),
        chunks[4],
    );
}

fn render_mem_compact(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" MEM ", Style::default().fg(theme::MEM_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:.1} / {:.1} GB", app.data.memory.used_gb, app.data.memory.total_gb),
                Style::default().fg(theme::TEXT_DIM),
            ),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_SURFACE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let show_swap = app.data.memory.swap_used_percent > 0.1;
    let swap_height = if show_swap { 1 } else { 0 };
    let sparkline_height = 1u16.min(inner.height.saturating_sub(1 + swap_height));
    let mut constraints = vec![Constraint::Length(1)];
    if show_swap { constraints.push(Constraint::Length(1)); }
    constraints.push(Constraint::Min(sparkline_height));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    render_metric_bar(f, chunks[0], "Used", app.data.memory.used_percent, 100.0, "%", theme::MEM_COLOR);
    if show_swap {
        render_metric_bar(f, chunks[1], "Swap", app.data.memory.swap_used_percent, 100.0, "%", theme::MEM_COLOR);
    }

    let spark_data = app.history.memory_used.as_slice();
    let spark_idx = if show_swap { 2 } else { 1 };
    f.render_widget(
        Sparkline::default().data(spark_data).max(100).style(Style::default().fg(theme::MEM_COLOR)),
        chunks[spark_idx],
    );
}

fn render_net_compact(f: &mut Frame, area: Rect, app: &App) {
    let iface_name = app.data.network.interfaces.first()
        .map(|i| i.name.clone())
        .unwrap_or_default();

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" NET ", Style::default().fg(theme::NET_TX_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(
                truncate(&iface_name, area.width.saturating_sub(10) as usize),
                Style::default().fg(theme::TEXT_DIM),
            ),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_SURFACE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let sparkline_height = 1u16.min(inner.height.saturating_sub(2));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(sparkline_height),
        ])
        .split(inner);

    let (rx, tx) = app.data.network.interfaces.first()
        .map(|i| (i.rx_bytes_per_sec, i.tx_bytes_per_sec))
        .unwrap_or((0.0, 0.0));

    let rx_line = Line::from(vec![
        Span::styled("↓ ", Style::default().fg(theme::NET_RX_COLOR)),
        Span::styled(format_rate(rx), Style::default().fg(theme::TEXT_SECONDARY)),
    ]);
    f.render_widget(Paragraph::new(rx_line), chunks[0]);

    let tx_line = Line::from(vec![
        Span::styled("↑ ", Style::default().fg(theme::NET_TX_COLOR)),
        Span::styled(format_rate(tx), Style::default().fg(theme::TEXT_SECONDARY)),
    ]);
    f.render_widget(Paragraph::new(tx_line), chunks[1]);

    // Sparkline for RX
    let spark_data = app.history.net_rx.as_slice();
    let max_val = spark_data.iter().copied().max().unwrap_or(1).max(1);
    f.render_widget(
        Sparkline::default().data(spark_data).max(max_val).style(Style::default().fg(theme::NET_RX_COLOR)),
        chunks[2],
    );
}

fn render_metric_bar_custom(f: &mut Frame, area: Rect, label: &str, percent: f32, display_val: &str, color: ratatui::style::Color) {
    let label_w = 5u16;
    let val_w = display_val.len() as u16 + 1;

    if area.width < label_w + val_w + 2 {
        return;
    }

    let bar_w = area.width.saturating_sub(label_w + val_w);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(label_w),
            Constraint::Length(bar_w),
            Constraint::Length(val_w),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(label).style(Style::default().fg(theme::TEXT_DIM)),
        cols[0],
    );

    let bar_color = if percent > 90.0 { theme::CRIT_COLOR } else if percent > 80.0 { theme::WARN_COLOR } else { color };
    f.render_widget(
        ColorBar::new((percent as f64 / 100.0).clamp(0.0, 1.0), bar_color),
        cols[1],
    );

    f.render_widget(
        Paragraph::new(format!(" {display_val}")).style(Style::default().fg(theme::TEXT_SECONDARY)),
        cols[2],
    );
}

fn render_metric_bar(f: &mut Frame, area: Rect, label: &str, value: f32, max: f32, unit: &str, color: ratatui::style::Color) {
    let label_w = 5u16;
    let val_str = format!("{:.1}{unit}", value);
    let val_w = val_str.len() as u16 + 1;

    if area.width < label_w + val_w + 2 {
        return;
    }

    let bar_w = area.width.saturating_sub(label_w + val_w);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(label_w),
            Constraint::Length(bar_w),
            Constraint::Length(val_w),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(label).style(Style::default().fg(theme::TEXT_DIM)),
        cols[0],
    );

    let bar_color = if value / max * 100.0 > 90.0 { theme::CRIT_COLOR } else if value / max * 100.0 > 80.0 { theme::WARN_COLOR } else { color };
    f.render_widget(
        ColorBar::new((value as f64 / max as f64).clamp(0.0, 1.0), bar_color),
        cols[1],
    );

    f.render_widget(
        Paragraph::new(val_str).style(Style::default().fg(theme::TEXT_SECONDARY)),
        cols[2],
    );
}

fn format_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_000_000.0 {
        format!("{:.1} MB/s", bytes_per_sec / 1_000_000.0)
    } else if bytes_per_sec >= 1_000.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1_000.0)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

/// Simple bar widget that uses background colors instead of Gauge.
/// Filled portion: bg = accent color. Unfilled portion: bg = dim color.
struct ColorBar {
    ratio: f64,
    color: ratatui::style::Color,
}

impl ColorBar {
    fn new(ratio: f64, color: ratatui::style::Color) -> Self {
        Self { ratio: ratio.clamp(0.0, 1.0), color }
    }
}

impl Widget for ColorBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let filled = (area.width as f64 * self.ratio).round() as u16;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let cell = &mut buf[(x, y)];
                if x < area.left() + filled {
                    cell.set_char(' ').set_bg(self.color);
                } else {
                    cell.set_char(' ').set_bg(theme::BG_SURFACE2);
                }
            }
        }
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
