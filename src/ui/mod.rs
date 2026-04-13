pub mod area_chart;
pub mod bottom_strip;
pub mod candlestick;
pub mod chart;
pub mod cpu;
pub mod footer;
pub mod gpu;
pub mod header;
pub mod about;
pub mod help;
pub mod memory;
pub mod network_disk;
pub mod process_table;
pub mod sparkline_buf;
pub mod status;
pub mod theme;
pub mod ticker;
pub mod watchlist;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

use crate::app::{App, PanelFocus};

/// Master render function — v2 trading-terminal layout.
pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    match app.focus {
        PanelFocus::Dashboard => render_dashboard(f, size, app),
        PanelFocus::Processes => {
            // Full-screen process view (legacy, accessible via 'p')
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // status
                    Constraint::Min(10),  // process table
                    Constraint::Length(1), // footer
                ])
                .split(size);
            ticker::render(f, rect_1line(chunks[0]), app);
            process_table::render(f, chunks[1], app);
            status::render(f, chunks[2], app);
        }
    }

    if app.show_help {
        help::render(f);
    }
    if app.show_about {
        about::render(f);
    }
}

fn render_dashboard(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    // Main vertical layout: ticker | main area | bottom strip | status bar
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // ticker bar
            Constraint::Min(10),   // main area (chart + watchlist)
            Constraint::Length(8),  // bottom strip (compact panels)
            Constraint::Length(1),  // status bar
        ])
        .split(area);

    ticker::render(f, main_chunks[0], app);
    status::render(f, main_chunks[3], app);

    // Main area: chart panel (left) + watchlist (right)
    let wide_enough = area.width >= 100;
    if wide_enough {
        let main_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(app.split_pct),
                Constraint::Percentage(100 - app.split_pct),
            ])
            .split(main_chunks[1]);

        chart::render(f, main_area[0], app);
        watchlist::render(f, main_area[1], app);
    } else {
        // Narrow terminal: stack chart above watchlist
        let main_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(main_chunks[1]);

        chart::render(f, main_area[0], app);
        watchlist::render(f, main_area[1], app);
    }

    // Bottom strip: compact 4-panel row
    bottom_strip::render(f, main_chunks[2], app);
}

/// Helper to create a 1-line rect from a larger area (for full-screen process view ticker)
fn rect_1line(area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    ratatui::layout::Rect::new(area.x, area.y, area.width, area.height.min(2))
}
