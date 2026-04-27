use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, ChartMode, ChartTab};
use ratatui::style::Color;

use crate::ui::area_chart::AreaChart;
use crate::ui::candlestick::CandlestickChart;
use crate::ui::horizon_chart::HorizonChart;
use crate::ui::theme;

/// Render the main chart panel with metric tabs.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_SURFACE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 4 || inner.width < 20 {
        return;
    }

    // Split: tab bar (1 line) + chart meta (1 line) + threshold legend (1 line) + chart body
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar + mode badge
            Constraint::Length(1), // chart meta (value + hw name + delta)
            Constraint::Min(3),   // chart body
        ])
        .split(inner);

    render_tabs(f, chunks[0], app);
    render_meta(f, chunks[1], app);
    render_chart_body(f, chunks[2], app);

    // Threshold legend (overlay in top-right of chart body)
    if matches!(app.chart_tab, ChartTab::Cpu | ChartTab::Gpu | ChartTab::Mem) {
        let legend_width = 22u16;
        if chunks[0].width > legend_width + 10 {
            let legend_area = Rect::new(
                chunks[0].x + chunks[0].width - legend_width,
                chunks[0].y,
                legend_width,
                1,
            );
            let legend = Paragraph::new(Line::from(vec![
                Span::styled("60s", Style::default().fg(theme::TEXT_DIM)),
                Span::styled(" · ", Style::default().fg(theme::BORDER2)),
                Span::styled("500ms", Style::default().fg(theme::TEXT_DIM)),
            ])).alignment(ratatui::layout::Alignment::Right);
            f.render_widget(legend, legend_area);
        }
    }
}

fn render_tabs(f: &mut Frame, area: Rect, app: &App) {
    let tabs = [
        (ChartTab::Cpu, "CPU", theme::CPU_COLOR),
        (ChartTab::Gpu, "GPU", theme::GPU_COLOR),
        (ChartTab::Mem, "MEM", theme::MEM_COLOR),
        (ChartTab::Net, "NET", theme::NET_TX_COLOR),
        (ChartTab::Disk, "DISK", theme::DISK_COLOR),
    ];

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::raw(" "));
    for (tab, label, color) in &tabs {
        if *tab == app.chart_tab {
            spans.push(Span::styled(
                format!("[{label}]"),
                Style::default().fg(*color).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {label} "),
                Style::default().fg(theme::TEXT_DIM),
            ));
        }
        spans.push(Span::raw(" "));
    }

    // Chart mode badge
    let mode = match app.chart_mode {
        ChartMode::Default => match app.chart_tab {
            ChartTab::Cpu => "CANDLE",
            _ => "AREA",
        },
        ChartMode::Horizon => "HORIZON",
    };
    let mode_color = match app.chart_mode {
        ChartMode::Horizon => theme::CPU_COLOR,
        ChartMode::Default => match app.chart_tab {
            ChartTab::Cpu => theme::CPU_COLOR,
            _ => theme::TEXT_DIM,
        },
    };
    spans.push(Span::styled(
        format!(" {mode} "),
        Style::default().fg(mode_color),
    ));

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_meta(f: &mut Frame, area: Rect, app: &App) {
    let (value_str, hw_str, color) = match app.chart_tab {
        ChartTab::Cpu => (
            format!("{:.1}%", app.data.cpu.total_load),
            format!("{} · {}-Core", app.data.cpu.name, app.data.cpu.per_core_load.len()),
            theme::CPU_COLOR,
        ),
        ChartTab::Gpu => {
            if let Some(gpu) = app.primary_gpu() {
                (
                    format!("{:.1}%", gpu.utilization),
                    gpu.name.clone(),
                    theme::GPU_COLOR,
                )
            } else {
                (
                    "N/A".to_string(),
                    dofek::gpu_empty_state().title.clone(),
                    theme::TEXT_DIM,
                )
            }
        }
        ChartTab::Mem => (
            format!("{:.1}%", app.data.memory.used_percent),
            format!("{:.1} / {:.1} GB", app.data.memory.used_gb, app.data.memory.total_gb),
            theme::MEM_COLOR,
        ),
        ChartTab::Net => {
            let (rx, _tx) = app.data.network.interfaces.first()
                .map(|i| (i.rx_bytes_per_sec, i.tx_bytes_per_sec))
                .unwrap_or((0.0, 0.0));
            (
                format_rate(rx),
                app.data.network.interfaces.first()
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| "No interface".to_string()),
                theme::NET_TX_COLOR,
            )
        }
        ChartTab::Disk => {
            let read = app.data.disk.total_read_bytes_per_sec;
            let write = app.data.disk.total_write_bytes_per_sec;
            let label = if app.data.disk.devices.is_empty() {
                "No disks".to_string()
            } else {
                format!("{} dev · ↑{}", app.data.disk.devices.len(), format_rate(write))
            };
            (
                format!("↓{}", format_rate(read)),
                label,
                theme::DISK_COLOR,
            )
        }
    };

    let mut spans = vec![
        Span::styled(
            format!(" {value_str}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {hw_str}"), Style::default().fg(theme::TEXT_DIM)),
    ];

    // Threshold legend (right side of meta line)
    if matches!(app.chart_tab, ChartTab::Cpu | ChartTab::Gpu | ChartTab::Mem) {
        // Pad to push legend right
        let used = value_str.len() + hw_str.len() + 3;
        let avail = area.width as usize;
        let legend = "— warn 80%  — crit 90%";
        if avail > used + legend.len() + 2 {
            let pad = avail - used - legend.len() - 2;
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled("— ", Style::default().fg(theme::WARN_COLOR)));
            spans.push(Span::styled("warn 80%  ", Style::default().fg(theme::TEXT_DIM)));
            spans.push(Span::styled("— ", Style::default().fg(theme::CRIT_COLOR)));
            spans.push(Span::styled("crit 90%", Style::default().fg(theme::TEXT_DIM)));
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_chart_body(f: &mut Frame, area: Rect, app: &App) {
    if app.chart_mode == ChartMode::Horizon {
        render_horizon(f, area, app);
        return;
    }

    match app.chart_tab {
        ChartTab::Cpu => {
            // Candlestick chart from candle buffer (needs 2+ candles)
            let candle_data = app.history.cpu_candle.as_slice();
            if candle_data.len() >= 2 {
                let chart = CandlestickChart::new(candle_data, theme::CPU_COLOR)
                    .thresholds(0.8, 0.9);
                f.render_widget(chart, area);
            } else {
                // Fallback: area chart from sparkline data while candles accumulate
                let data = app.history.cpu_total.as_slice();
                if data.len() >= 2 {
                    let chart = AreaChart::new(data, theme::CPU_COLOR)
                        .max_value(100)
                        .thresholds(0.8, 0.9);
                    f.render_widget(chart, area);
                } else {
                    // Show placeholder centered in chart area while collecting initial data
                    let rows = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Fill(1),
                            Constraint::Length(1),
                            Constraint::Fill(1),
                        ])
                        .split(area);
                    let msg = Paragraph::new(format!("Please Wait... Collecting data. ({} samples)", data.len()))
                        .style(Style::default().fg(theme::TEXT_DIM))
                        .alignment(ratatui::layout::Alignment::Center);
                    f.render_widget(msg, rows[1]);
                }
            }
        }
        ChartTab::Gpu => {
            let data = app.history.gpu_util.as_slice();
            // If multi-GPU, overlay secondary series
            if app.history.gpu_util_per_device.len() > 1 {
                let primary = app.history.gpu_util_per_device[0].as_slice();
                let secondary = app.history.gpu_util_per_device[1].as_slice();
                let chart = AreaChart::new(primary, theme::GPU_COLOR)
                    .max_value(100)
                    .secondary(secondary, Color::Rgb(0xDB, 0x27, 0x77)) // pink
                    .thresholds(0.85, 0.95);
                f.render_widget(chart, area);
            } else {
                let chart = AreaChart::new(data, theme::GPU_COLOR)
                    .max_value(100)
                    .thresholds(0.85, 0.95);
                f.render_widget(chart, area);
            }
        }
        ChartTab::Mem => {
            let data = app.history.memory_used.as_slice();
            let chart = AreaChart::new(data, theme::MEM_COLOR)
                .max_value(100)
                .thresholds(0.8, 0.9);
            f.render_widget(chart, area);
        }
        ChartTab::Net => {
            let rx_data = app.history.net_rx.as_slice();
            let tx_data = app.history.net_tx.as_slice();
            let max_val = rx_data.iter().chain(tx_data.iter()).copied().max().unwrap_or(1).max(1);
            let chart = AreaChart::new(rx_data, theme::NET_RX_COLOR)
                .max_value(max_val)
                .secondary(tx_data, theme::NET_TX_COLOR);
            f.render_widget(chart, area);
        }
        ChartTab::Disk => {
            let read_data = app.history.disk_read.as_slice();
            let write_data = app.history.disk_write.as_slice();
            let max_val = read_data.iter().chain(write_data.iter()).copied().max().unwrap_or(1).max(1);
            let chart = AreaChart::new(read_data, theme::DISK_COLOR)
                .max_value(max_val)
                .secondary(write_data, theme::NET_TX_COLOR);
            f.render_widget(chart, area);
        }
    }
}

fn render_horizon(f: &mut Frame, area: Rect, app: &App) {
    match app.chart_tab {
        ChartTab::Cpu => {
            let data = app.history.cpu_total.as_slice();
            if data.len() >= 2 {
                let chart = HorizonChart::new(data, theme::CPU_COLOR)
                    .max_value(100)
                    .thresholds(0.8, 0.9);
                f.render_widget(chart, area);
            }
        }
        ChartTab::Gpu => {
            let data = app.history.gpu_util.as_slice();
            if data.len() >= 2 {
                let chart = HorizonChart::new(data, theme::GPU_COLOR)
                    .max_value(100)
                    .thresholds(0.85, 0.95);
                f.render_widget(chart, area);
            }
        }
        ChartTab::Mem => {
            let data = app.history.memory_used.as_slice();
            if data.len() >= 2 {
                let chart = HorizonChart::new(data, theme::MEM_COLOR)
                    .max_value(100)
                    .thresholds(0.8, 0.9);
                f.render_widget(chart, area);
            }
        }
        ChartTab::Net => {
            let rx_data = app.history.net_rx.as_slice();
            let tx_data = app.history.net_tx.as_slice();
            let max_val = rx_data.iter().chain(tx_data.iter()).copied().max().unwrap_or(1).max(1);
            if rx_data.len() >= 2 {
                let chart = HorizonChart::new(rx_data, theme::NET_RX_COLOR)
                    .max_value(max_val);
                f.render_widget(chart, area);
            }
        }
        ChartTab::Disk => {
            let read_data = app.history.disk_read.as_slice();
            let write_data = app.history.disk_write.as_slice();
            let max_val = read_data.iter().chain(write_data.iter()).copied().max().unwrap_or(1).max(1);
            if read_data.len() >= 2 {
                let chart = HorizonChart::new(read_data, theme::DISK_COLOR)
                    .max_value(max_val);
                f.render_widget(chart, area);
            }
        }
    }
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
