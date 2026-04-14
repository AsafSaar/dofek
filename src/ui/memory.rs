use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline, Widget};
use ratatui::Frame;

use crate::app::App;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mem = &app.data.memory;

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" MEM ", Style::default().fg(theme::MEM_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:.1} / {:.1} GB", mem.used_gb, mem.total_gb),
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

    let sparkline_height = 3.min(inner.height.saturating_sub(3));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(sparkline_height),
        ])
        .split(inner);

    // Memory bars
    let bar_area = chunks[0];
    let bar_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(bar_area);

    // Used memory bar
    render_bar(f, bar_rows[0], "Used", mem.used_percent, theme::MEM_COLOR);

    // Swap bar
    render_bar(f, bar_rows[1], "Swap", mem.swap_used_percent, theme::ACCENT_AMBER);

    // Sparkline
    let spark_data = app.history.memory_used.as_slice();
    let sparkline = Sparkline::default()
        .data(spark_data)
        .max(100)
        .style(Style::default().fg(theme::MEM_COLOR));
    f.render_widget(sparkline, chunks[1]);
}

fn render_bar(f: &mut Frame, area: Rect, label: &str, percent: f32, color: ratatui::style::Color) {
    let label_width = 5u16;
    let value_width = 7u16;

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

    f.render_widget(ColorBar::new((percent as f64 / 100.0).clamp(0.0, 1.0), color), cols[1]);

    f.render_widget(
        Paragraph::new(format!("{:5.1}%", percent)).style(Style::default().fg(theme::TEXT_PRIMARY)),
        cols[2],
    );
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
