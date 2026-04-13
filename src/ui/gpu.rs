use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline, Widget};
use ratatui::Frame;

use crate::app::App;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let gpu = app.primary_gpu();

    let title_detail = gpu.map(|g| {
        format!("{} · {:.0} MB", g.name, g.vram_total_mb)
    }).unwrap_or_else(|| "No GPU detected".to_string());

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" GPU ", Style::default().fg(theme::GPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(
                truncate(&title_detail, area.width.saturating_sub(20) as usize),
                Style::default().fg(theme::TEXT_SECONDARY),
            ),
            Span::raw(" "),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG_PANEL));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 3 {
        return;
    }

    let Some(gpu) = gpu else {
        let msg = if app.data.nvml_available {
            "No GPU data available"
        } else {
            "No NVIDIA GPU detected"
        };
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(theme::TEXT_DIM)),
            inner,
        );
        return;
    };

    let sparkline_height = 3.min(inner.height.saturating_sub(5));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(sparkline_height),
        ])
        .split(inner);

    // GPU metric bars
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(chunks[0]);

    render_bar(f, rows[0], "Util", gpu.utilization, 100.0, "%", theme::GPU_COLOR);

    let vram_pct = if gpu.vram_total_mb > 0.0 {
        gpu.vram_used_mb / gpu.vram_total_mb * 100.0
    } else {
        0.0
    };
    render_bar_with_value(
        f, rows[1], "VRAM",
        vram_pct,
        &format!("{:.0}/{:.0}MB", gpu.vram_used_mb, gpu.vram_total_mb),
        theme::VRAM_COLOR,
    );

    if app.config.display.show_temps {
        render_bar(f, rows[2], "Temp", gpu.temperature, 100.0, "°C", theme::ACCENT_AMBER);
    }

    if app.config.display.show_power {
        render_bar(f, rows[3], "Powr", gpu.power_watts, 350.0, "W", theme::ACCENT_AMBER);
    }

    // Sparkline
    let spark_data = app.history.gpu_util.as_slice();
    let max_val = spark_data.iter().copied().max().unwrap_or(1).max(1);
    let sparkline = Sparkline::default()
        .data(&spark_data)
        .max(max_val)
        .style(Style::default().fg(theme::GPU_COLOR));
    f.render_widget(sparkline, chunks[1]);
}

fn render_bar(f: &mut Frame, area: Rect, label: &str, value: f32, max: f32, unit: &str, color: ratatui::style::Color) {
    let label_width = 5u16;
    let value_str = format!("{:5.1}{}", value, unit);
    let value_width = value_str.len() as u16 + 1;

    if area.width < label_width + value_width + 3 {
        return;
    }

    let bar_width = area.width.saturating_sub(label_width + value_width);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(label_width),
            Constraint::Length(bar_width),
            Constraint::Length(value_width),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(label).style(Style::default().fg(theme::TEXT_SECONDARY)),
        cols[0],
    );

    let ratio = (value as f64 / max as f64).clamp(0.0, 1.0);
    f.render_widget(ColorBar::new(ratio, color), cols[1]);

    f.render_widget(
        Paragraph::new(value_str).style(Style::default().fg(theme::TEXT_PRIMARY)),
        cols[2],
    );
}

fn render_bar_with_value(f: &mut Frame, area: Rect, label: &str, percent: f32, value_str: &str, color: ratatui::style::Color) {
    let label_width = 5u16;
    let value_width = value_str.len() as u16 + 1;

    if area.width < label_width + value_width + 3 {
        return;
    }

    let bar_width = area.width.saturating_sub(label_width + value_width);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(label_width),
            Constraint::Length(bar_width),
            Constraint::Length(value_width),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(label).style(Style::default().fg(theme::TEXT_SECONDARY)),
        cols[0],
    );

    f.render_widget(
        ColorBar::new((percent as f64 / 100.0).clamp(0.0, 1.0), color),
        cols[1],
    );

    f.render_widget(
        Paragraph::new(value_str.to_string()).style(Style::default().fg(theme::TEXT_PRIMARY)),
        cols[2],
    );
}

/// Simple bar widget using background colors. Avoids Gauge rendering quirks.
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
