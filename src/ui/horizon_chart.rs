use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

use crate::ui::theme;

/// Horizon chart that folds the y-axis into layered color bands.
/// Low values show a dim band, medium values overlay a brighter band,
/// and high values add a vivid top band — stacking color intensity.
pub struct HorizonChart<'a> {
    data: &'a [u64],
    max_value: u64,
    color: Color,
    num_bands: usize,
    warn_threshold: Option<f64>,
    crit_threshold: Option<f64>,
}

impl<'a> HorizonChart<'a> {
    pub fn new(data: &'a [u64], color: Color) -> Self {
        Self {
            data,
            max_value: 100,
            color,
            num_bands: 3,
            warn_threshold: None,
            crit_threshold: None,
        }
    }

    pub fn max_value(mut self, max: u64) -> Self {
        self.max_value = max.max(1);
        self
    }

    pub fn thresholds(mut self, warn: f64, crit: f64) -> Self {
        self.warn_threshold = Some(warn);
        self.crit_threshold = Some(crit);
        self
    }
}

impl Widget for HorizonChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 2 || self.data.len() < 2 {
            return;
        }

        let w = area.width as usize;
        let h = area.height as usize;
        let sub_h = h * 2; // half-block vertical resolution
        let max = self.max_value as f64;
        let bands = theme::horizon_bands(self.color);
        let num_bands = self.num_bands.min(bands.len());
        let band_range = max / num_bands as f64;

        // Resample data to chart width (nearest-neighbor)
        let resampled: Vec<f64> = resample_f64(self.data, w, max);

        // Draw threshold lines behind the chart
        if let Some(warn) = self.warn_threshold {
            let y_sub = ((1.0 - warn) * sub_h as f64) as usize;
            draw_threshold_line(buf, area, y_sub, Color::Rgb(0xFB, 0xBF, 0x24));
        }
        if let Some(crit) = self.crit_threshold {
            let y_sub = ((1.0 - crit) * sub_h as f64) as usize;
            draw_threshold_line(buf, area, y_sub, Color::Rgb(0xF8, 0x71, 0x71));
        }

        // Draw horizon bands column by column
        // Each column fills to the value's TRUE position on the y-axis,
        // but uses different band colors for each band's portion of the fill.
        for (col_idx, &val) in resampled.iter().enumerate() {
            let x = area.x + col_idx as u16;
            if x >= area.x + area.width {
                break;
            }

            if val <= 0.0 {
                continue;
            }

            // Total fill height in sub-pixels (actual y-axis position)
            let total_frac = (val / max).clamp(0.0, 1.0);
            let total_fill = (total_frac * sub_h as f64).round() as usize;
            if total_fill == 0 {
                continue;
            }

            // Determine the band color for each sub-pixel from bottom to top
            // sub_y=0 is bottom, sub_y=total_fill-1 is top
            for sub_y in 0..total_fill {
                // What value does this sub-pixel represent?
                let pixel_val = (sub_y as f64 + 0.5) / sub_h as f64 * max;
                let band = ((pixel_val / band_range) as usize).min(num_bands - 1);
                let band_color = bands[band.min(bands.len() - 1)];

                let from_top = sub_h - 1 - sub_y;
                let row = from_top / 2;
                let is_top_half = from_top.is_multiple_of(2);
                let cy = area.y + row as u16;

                if cy >= area.y + area.height {
                    continue;
                }

                let cell = buf.cell_mut((x, cy)).unwrap();
                let ch = cell.symbol().chars().next().unwrap_or(' ');

                if is_top_half {
                    if ch == '█' || ch == '▄' {
                        cell.set_char('█');
                        cell.set_fg(band_color);
                    } else {
                        cell.set_char('▀');
                        cell.set_fg(band_color);
                    }
                } else {
                    if ch == '█' || ch == '▀' {
                        cell.set_char('█');
                        cell.set_fg(band_color);
                    } else {
                        cell.set_char('▄');
                        cell.set_fg(band_color);
                    }
                }
            }
        }

        // Y-axis labels
        draw_y_labels(buf, area, self.max_value);
    }
}

/// Resample data to exactly `target_len` f64 values, clamped to max.
fn resample_f64(data: &[u64], target_len: usize, max: f64) -> Vec<f64> {
    if data.is_empty() {
        return vec![0.0; target_len];
    }
    (0..target_len)
        .map(|i| {
            let src_idx = (i as f64 / target_len as f64 * data.len() as f64) as usize;
            let src_idx = src_idx.min(data.len() - 1);
            (data[src_idx] as f64).clamp(0.0, max)
        })
        .collect()
}

/// Draw a horizontal threshold line (dashed pattern).
fn draw_threshold_line(buf: &mut Buffer, area: Rect, y_sub: usize, color: Color) {
    let row = y_sub / 2;
    let cy = area.y + row as u16;
    if cy >= area.y + area.height {
        return;
    }

    let dim_col = dim(color, 0.4);
    for col in 0..area.width {
        let x = area.x + col;
        if (col as usize / 3).is_multiple_of(2) {
            let cell = buf.cell_mut((x, cy)).unwrap();
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

    let labels: Vec<(f64, &str)> = if max_value <= 100 {
        vec![(0.0, "0"), (0.25, "25"), (0.5, "50"), (0.75, "75"), (1.0, "100")]
    } else {
        return;
    };

    let label_color = Color::Rgb(0x3D, 0x50, 0x70);

    for (frac, text) in labels {
        let y_row = ((1.0 - frac) * (area.height as f64 - 1.0)) as u16;
        let cy = area.y + y_row;
        if cy < area.y + area.height {
            let x_start = area.x + area.width - text.len() as u16;
            buf.set_string(x_start, cy, text, Style::default().fg(label_color));
        }
    }
}

/// Dim a color by a factor (0..1).
fn dim(color: Color, factor: f64) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f64 * factor) as u8,
            (g as f64 * factor) as u8,
            (b as f64 * factor) as u8,
        ),
        other => other,
    }
}
