use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline, Widget};
use ratatui::Frame;

use crate::app::App;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" CPU ", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(
                truncate_str(&app.data.cpu.name, area.width.saturating_sub(20) as usize),
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

    // Split: core bars on top, sparkline at bottom
    let sparkline_height = 3.min(inner.height.saturating_sub(2));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(sparkline_height),
        ])
        .split(inner);

    // Per-core bars
    render_core_bars(f, chunks[0], app);

    // Sparkline
    let spark_data = app.history.cpu_total.as_slice();
    let sparkline = Sparkline::default()
        .data(spark_data)
        .max(100)
        .style(Style::default().fg(theme::CPU_COLOR));
    f.render_widget(sparkline, chunks[1]);
}

fn render_core_bars(f: &mut Frame, area: Rect, app: &App) {
    let cores = &app.data.cpu.per_core_load;
    if cores.is_empty() {
        let msg = "Waiting for data...";
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(theme::TEXT_DIM)),
            area,
        );
        return;
    }

    // Show as many cores as we have vertical space for
    let max_cores = area.height as usize;
    let displayed_cores = cores.len().min(max_cores);

    let constraints: Vec<Constraint> = (0..displayed_cores)
        .map(|_| Constraint::Length(1))
        .collect();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (i, row) in rows.iter().enumerate() {
        if i >= cores.len() {
            break;
        }
        let pct = cores[i];
        let label = format!("C{:<2}", i);
        let value = format!("{:5.1}%", pct);

        let label_width = 4u16;
        let value_width = 7u16;
        let bar_width = row.width.saturating_sub(label_width + value_width + 1);

        if row.width < label_width + value_width + 3 {
            continue;
        }

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(label_width),
                Constraint::Length(bar_width),
                Constraint::Length(value_width),
            ])
            .split(*row);

        f.render_widget(
            Paragraph::new(label).style(Style::default().fg(theme::TEXT_SECONDARY)),
            cols[0],
        );

        f.render_widget(ColorBar::new((pct as f64 / 100.0).clamp(0.0, 1.0), theme::CPU_COLOR), cols[1]);

        f.render_widget(
            Paragraph::new(value).style(Style::default().fg(theme::TEXT_PRIMARY)),
            cols[2],
        );
    }
}

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

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}
