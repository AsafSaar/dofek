pub mod cpu;
pub mod footer;
pub mod gpu;
pub mod header;
pub mod help;
pub mod memory;
pub mod network_disk;
pub mod process_table;
pub mod sparkline_buf;
pub mod theme;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

use crate::app::{App, PanelFocus};

/// Master render function — dispatches to panel renderers based on focus state.
pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    // Main vertical layout: header, body, footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // header
            Constraint::Min(10),   // body
            Constraint::Length(1),  // footer
        ])
        .split(size);

    header::render(f, main_chunks[0], app);
    footer::render(f, main_chunks[2], app);

    match app.focus {
        PanelFocus::Dashboard => render_dashboard(f, main_chunks[1], app),
        PanelFocus::Cpu => cpu::render(f, main_chunks[1], app),
        PanelFocus::Memory => memory::render(f, main_chunks[1], app),
        PanelFocus::Gpu => gpu::render(f, main_chunks[1], app),
        PanelFocus::Processes => process_table::render(f, main_chunks[1], app),
    }

    if app.show_help {
        help::render(f);
    }
}

fn render_dashboard(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    // Split body into top panels and bottom process table
    let body_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35), // top row: CPU + Memory
            Constraint::Percentage(30), // middle row: GPU + Network
            Constraint::Min(5),         // bottom: process table
        ])
        .split(area);

    // Top row: CPU | Memory
    let top_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(body_chunks[0]);

    cpu::render(f, top_row[0], app);
    memory::render(f, top_row[1], app);

    // Middle row: GPU | Network+Disk
    let mid_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(body_chunks[1]);

    gpu::render(f, mid_row[0], app);
    network_disk::render(f, mid_row[1], app);

    // Bottom: Process table
    process_table::render(f, body_chunks[2], app);
}
