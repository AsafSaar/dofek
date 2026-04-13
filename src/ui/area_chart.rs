use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

/// Area chart that fills the entire rendering area using half-block characters.
/// Renders a line with filled area below, threshold lines, and an optional secondary series.
pub struct AreaChart<'a> {
    data: &'a [u64],
    max_value: u64,
    color: Color,
    secondary: Option<(&'a [u64], Color)>,
    warn_threshold: Option<f64>,  // as fraction 0..1
    crit_threshold: Option<f64>,
}

impl<'a> AreaChart<'a> {
    pub fn new(data: &'a [u64], color: Color) -> Self {
        Self {
            data,
            max_value: 100,
            color,
            secondary: None,
            warn_threshold: None,
            crit_threshold: None,
        }
    }

    pub fn max_value(mut self, max: u64) -> Self {
        self.max_value = max.max(1);
        self
    }

    pub fn secondary(mut self, data: &'a [u64], color: Color) -> Self {
        self.secondary = Some((data, color));
        self
    }

    pub fn thresholds(mut self, warn: f64, crit: f64) -> Self {
        self.warn_threshold = Some(warn);
        self.crit_threshold = Some(crit);
        self
    }
}

impl Widget for AreaChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 3 || self.data.len() < 2 {
            return;
        }

        let w = area.width as usize;
        let h = area.height as usize;
        let sub_h = h * 2; // half-block resolution

        let max = self.max_value as f64;

        // Map data to sub-pixel Y positions (0 = top, sub_h = bottom)
        let map_y = |val: u64| -> usize {
            let frac = (val as f64 / max).clamp(0.0, 1.0);
            let y = ((1.0 - frac) * sub_h as f64) as usize;
            y.min(sub_h.saturating_sub(1))
        };

        // Resample data to fit the chart width
        let data_points: Vec<usize> = resample(self.data, w, |v| map_y(v));

        // Draw threshold lines first (behind the chart)
        if let Some(warn) = self.warn_threshold {
            let y_sub = ((1.0 - warn) * sub_h as f64) as usize;
            draw_threshold_line(buf, area, y_sub, Color::Rgb(0xFB, 0xBF, 0x24), sub_h);
        }
        if let Some(crit) = self.crit_threshold {
            let y_sub = ((1.0 - crit) * sub_h as f64) as usize;
            draw_threshold_line(buf, area, y_sub, Color::Rgb(0xF8, 0x71, 0x71), sub_h);
        }

        // Draw secondary series (if any) — line only, no fill
        if let Some((sec_data, sec_color)) = self.secondary {
            let sec_points: Vec<usize> = resample(sec_data, w, |v| map_y(v));
            draw_line_only(buf, area, &sec_points, sec_color, sub_h);
        }

        // Draw primary series — filled area + line
        draw_filled_area(buf, area, &data_points, self.color, sub_h);

        // Y-axis labels
        draw_y_labels(buf, area, self.max_value);
    }
}

/// Resample data to exactly `target_len` points using nearest-neighbor.
fn resample(data: &[u64], target_len: usize, map: impl Fn(u64) -> usize) -> Vec<usize> {
    if data.is_empty() {
        return vec![0; target_len];
    }
    (0..target_len).map(|i| {
        let src_idx = (i as f64 / target_len as f64 * data.len() as f64) as usize;
        let src_idx = src_idx.min(data.len() - 1);
        map(data[src_idx])
    }).collect()
}

/// Draw a filled area chart using half-block characters.
fn draw_filled_area(buf: &mut Buffer, area: Rect, points: &[usize], color: Color, _sub_h: usize) {
    let dim_color = dim(color, 0.25);

    for (col_idx, &y_sub) in points.iter().enumerate() {
        let x = area.x + col_idx as u16;
        if x >= area.x + area.width {
            break;
        }

        // For each cell row, determine what to draw
        for row in 0..area.height as usize {
            let cell_top = row * 2;      // top sub-pixel of this cell
            let cell_bot = row * 2 + 1;  // bottom sub-pixel of this cell
            let cy = area.y + row as u16;

            let top_filled = cell_top >= y_sub;
            let bot_filled = cell_bot >= y_sub;
            let top_is_line = cell_top == y_sub || (cell_top + 1 == y_sub && !top_filled);
            let bot_is_line = cell_bot == y_sub;

            if top_filled && bot_filled {
                // Full cell filled (area below line)
                let cell = buf.cell_mut((x, cy)).unwrap();
                cell.set_char('█');
                cell.set_fg(dim_color);
            } else if !top_filled && bot_filled {
                // Bottom half filled — line passes through top half
                let cell = buf.cell_mut((x, cy)).unwrap();
                cell.set_char('▄');
                cell.set_fg(if bot_is_line { color } else { dim_color });
                cell.set_bg(Color::Reset);
            } else if top_filled && !bot_filled {
                // Top half filled
                let cell = buf.cell_mut((x, cy)).unwrap();
                cell.set_char('▀');
                cell.set_fg(if top_is_line { color } else { dim_color });
                cell.set_bg(Color::Reset);
            } else if y_sub == cell_top {
                // Line exactly at top of cell
                let cell = buf.cell_mut((x, cy)).unwrap();
                cell.set_char('▀');
                cell.set_fg(color);
                cell.set_bg(Color::Reset);
            } else if y_sub == cell_bot {
                // Line exactly at bottom of cell
                let cell = buf.cell_mut((x, cy)).unwrap();
                cell.set_char('▄');
                cell.set_fg(color);
                cell.set_bg(Color::Reset);
            }
            // else: empty cell above the line, leave as-is
        }
    }

    // Draw brighter line on top
    for (col_idx, &y_sub) in points.iter().enumerate() {
        let x = area.x + col_idx as u16;
        if x >= area.x + area.width {
            break;
        }
        let row = y_sub / 2;
        let is_top = y_sub % 2 == 0;
        let cy = area.y + row as u16;
        if cy < area.y + area.height {
            let cell = buf.cell_mut((x, cy)).unwrap();
            if is_top {
                cell.set_char('▀');
                cell.set_fg(color);
            } else {
                cell.set_char('▄');
                cell.set_fg(color);
            }
        }
    }
}

/// Draw a line-only series (no fill) for overlays.
fn draw_line_only(buf: &mut Buffer, area: Rect, points: &[usize], color: Color, _sub_h: usize) {
    for (col_idx, &y_sub) in points.iter().enumerate() {
        let x = area.x + col_idx as u16;
        if x >= area.x + area.width {
            break;
        }
        let row = y_sub / 2;
        let is_top = y_sub % 2 == 0;
        let cy = area.y + row as u16;
        if cy < area.y + area.height {
            let cell = buf.cell_mut((x, cy)).unwrap();
            if is_top {
                cell.set_char('▀');
            } else {
                cell.set_char('▄');
            }
            cell.set_fg(color);
        }
    }
}

/// Draw a horizontal threshold line (dashed pattern).
fn draw_threshold_line(buf: &mut Buffer, area: Rect, y_sub: usize, color: Color, _sub_h: usize) {
    let row = y_sub / 2;
    let cy = area.y + row as u16;
    if cy >= area.y + area.height {
        return;
    }

    let dim_col = dim(color, 0.4);
    for col in 0..area.width {
        let x = area.x + col;
        // Dashed: draw every other 3 chars
        if (col as usize / 3) % 2 == 0 {
            let cell = buf.cell_mut((x, cy)).unwrap();
            // Don't overwrite existing chart data
            if cell.symbol() == " " {
                cell.set_char('╌');
                cell.set_fg(dim_col);
            }
        }
    }
}

/// Draw Y-axis labels on the right edge.
fn draw_y_labels(buf: &mut Buffer, area: Rect, max_value: u64) {
    if area.height < 4 || area.width < 6 {
        return;
    }

    let labels = if max_value <= 100 {
        vec![(0.0, "0"), (0.25, "25"), (0.5, "50"), (0.75, "75"), (1.0, "100")]
    } else {
        return; // Skip labels for non-percentage data
    };

    let label_color = Color::Rgb(0x3D, 0x50, 0x70); // TEXT_DIM

    for (frac, text) in labels {
        let y_row = ((1.0 - frac) * (area.height as f64 - 1.0)) as u16;
        let cy = area.y + y_row;
        if cy < area.y + area.height {
            // Draw on the right edge
            let x_start = area.x + area.width - text.len() as u16;
            buf.set_string(x_start, cy, text, Style::default().fg(label_color));
        }
    }
}

/// Dim a color by a factor (0..1).
fn dim(color: Color, factor: f64) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            Color::Rgb(
                (r as f64 * factor) as u8,
                (g as f64 * factor) as u8,
                (b as f64 * factor) as u8,
            )
        }
        other => other,
    }
}
