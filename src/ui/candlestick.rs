use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;
use std::collections::VecDeque;

use crate::ui::sparkline_buf::CandleSample;

/// Candlestick chart rendered via direct Buffer manipulation.
/// Each candle shows: wick (min-max), body (p25-p75), mean tick.
pub struct CandlestickChart<'a> {
    data: &'a VecDeque<CandleSample>,
    color: Color,
    warn_threshold: Option<f64>,
    crit_threshold: Option<f64>,
}

impl<'a> CandlestickChart<'a> {
    pub fn new(data: &'a VecDeque<CandleSample>, color: Color) -> Self {
        Self {
            data,
            color,
            warn_threshold: None,
            crit_threshold: None,
        }
    }

    pub fn thresholds(mut self, warn: f64, crit: f64) -> Self {
        self.warn_threshold = Some(warn);
        self.crit_threshold = Some(crit);
        self
    }
}

impl Widget for CandlestickChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 6 || area.height < 4 || self.data.len() < 2 {
            return;
        }

        let w = area.width as usize;
        let h = area.height as usize;
        let sub_h = h * 2; // half-block vertical resolution

        // Each candle is 1 column thin with 1 column gap (trading-terminal style)
        let candle_width = 1usize;
        let gap = 1usize;
        let slot_width = candle_width + gap;
        let max_candles = w / slot_width;
        let n = self.data.len().min(max_candles);
        let start_idx = self.data.len().saturating_sub(n);

        // Start drawing from right side
        let start_x = w.saturating_sub(n * slot_width);

        // Map value (0..100) to sub-pixel Y (0=top, sub_h-1=bottom)
        let map_y = |val: f32| -> usize {
            let frac = (val / 100.0).clamp(0.0, 1.0);
            let y = ((1.0 - frac) * (sub_h as f32 - 1.0)) as usize;
            y.min(sub_h.saturating_sub(1))
        };

        // Draw threshold lines first
        if let Some(warn) = self.warn_threshold {
            draw_dashed_line(buf, area, map_y(warn as f32 * 100.0), Color::Rgb(0xFB, 0xBF, 0x24));
        }
        if let Some(crit) = self.crit_threshold {
            draw_dashed_line(buf, area, map_y(crit as f32 * 100.0), Color::Rgb(0xF8, 0x71, 0x71));
        }

        // Draw grid lines (subtle)
        for pct in [25, 50, 75] {
            let y_sub = map_y(pct as f32);
            let row = y_sub / 2;
            let cy = area.y + row as u16;
            if cy < area.y + area.height {
                for col in 0..area.width {
                    let x = area.x + col;
                    if (col as usize).is_multiple_of(4) {
                        let cell = buf.cell_mut((x, cy)).unwrap();
                        if cell.symbol() == " " {
                            cell.set_char('·');
                            cell.set_fg(Color::Rgb(0x15, 0x1C, 0x2E));
                        }
                    }
                }
            }
        }

        let wick_color = dim(self.color, 0.35);
        let body_color = dim(self.color, 0.5);
        let bright_body = dim(self.color, 0.8);

        // Draw each candle (iterate from start_idx, no Vec allocation)
        for (i, candle) in self.data.iter().skip(start_idx).enumerate() {
            let cx = area.x + (start_x + i * slot_width + candle_width / 2) as u16;
            if cx >= area.x + area.width {
                break;
            }

            let is_last = i == n - 1;
            let y_min = map_y(candle.min);
            let y_max = map_y(candle.max);
            let y_p25 = map_y(candle.p25);
            let y_p75 = map_y(candle.p75);
            let y_mean = map_y(candle.mean);

            // Wick: vertical line from max to min (max is higher value = lower y)
            let wick_top = y_max.min(y_min); // higher value = lower sub-pixel
            let wick_bot = y_max.max(y_min);
            for sub_y in wick_top..=wick_bot {
                let row = sub_y / 2;
                let cy = area.y + row as u16;
                if cy < area.y + area.height {
                    let cell = buf.cell_mut((cx, cy)).unwrap();
                    if cell.symbol() == " " || cell.symbol() == "·" {
                        cell.set_char('│');
                        cell.set_fg(wick_color);
                    }
                }
            }

            // Body: filled rectangle from p75 to p25
            let body_top = y_p75.min(y_p25);
            let body_bot = y_p75.max(y_p25);
            let cur_body_color = if is_last { bright_body } else { body_color };

            // Draw body on center column and one column to each side
            for dx in 0..candle_width {
                let bx = area.x + (start_x + i * slot_width + dx) as u16;
                if bx >= area.x + area.width {
                    break;
                }

                for sub_y in body_top..=body_bot {
                    let row = sub_y / 2;
                    let is_top_half = sub_y % 2 == 0;
                    let cy = area.y + row as u16;
                    if cy < area.y + area.height {
                        let cell = buf.cell_mut((bx, cy)).unwrap();
                        // Check if both halves of this cell are in the body
                        let other_half = if is_top_half { sub_y + 1 } else { sub_y - 1 };
                        let other_in_body = other_half >= body_top && other_half <= body_bot;

                        if other_in_body {
                            cell.set_char('█');
                            cell.set_fg(cur_body_color);
                        } else if is_top_half {
                            cell.set_char('▀');
                            cell.set_fg(cur_body_color);
                        } else {
                            cell.set_char('▄');
                            cell.set_fg(cur_body_color);
                        }
                    }
                }
            }

            // Mean tick: horizontal mark at mean value
            let mean_row = y_mean / 2;
            let mean_cy = area.y + mean_row as u16;
            if mean_cy < area.y + area.height {
                let mean_color = if is_last { self.color } else { dim(self.color, 0.7) };
                for dx in 0..candle_width {
                    let mx = area.x + (start_x + i * slot_width + dx) as u16;
                    if mx < area.x + area.width {
                        let cell = buf.cell_mut((mx, mean_cy)).unwrap();
                        cell.set_char('─');
                        cell.set_fg(mean_color);
                    }
                }
            }
        }

        // Live mean guide: dashed horizontal line at the last candle's mean
        if let Some(last) = self.data.back() {
            let y_sub = map_y(last.mean);
            draw_dashed_line(buf, area, y_sub, dim(self.color, 0.3));
        }

        // Live dot on last candle
        if let Some(last) = self.data.back() {
            let y_sub = map_y(last.mean);
            let row = y_sub / 2;
            let cy = area.y + row as u16;
            let cx = area.x + area.width - 2;
            if cy < area.y + area.height && cx >= area.x {
                let cell = buf.cell_mut((cx, cy)).unwrap();
                cell.set_char('●');
                cell.set_fg(self.color);
            }
        }

        // Y-axis labels
        draw_y_labels(buf, area);
    }
}

fn draw_dashed_line(buf: &mut Buffer, area: Rect, y_sub: usize, color: Color) {
    let row = y_sub / 2;
    let cy = area.y + row as u16;
    if cy >= area.y + area.height {
        return;
    }

    for col in 0..area.width {
        let x = area.x + col;
        if (col as usize / 3).is_multiple_of(2) {
            let cell = buf.cell_mut((x, cy)).unwrap();
            if cell.symbol() == " " || cell.symbol() == "·" {
                cell.set_char('╌');
                cell.set_fg(dim(color, 0.5));
            }
        }
    }
}

fn draw_y_labels(buf: &mut Buffer, area: Rect) {
    if area.height < 5 || area.width < 6 {
        return;
    }

    let label_color = Color::Rgb(0x3D, 0x50, 0x70);
    let labels = [(1.0, "100"), (0.75, "75"), (0.5, "50"), (0.25, "25"), (0.0, "0")];

    for (frac, text) in labels {
        let y_row = ((1.0 - frac) * (area.height as f64 - 1.0)) as u16;
        let cy = area.y + y_row;
        if cy < area.y + area.height {
            let x_start = area.x + area.width - text.len() as u16;
            buf.set_string(x_start, cy, text, Style::default().fg(label_color));
        }
    }
}

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
