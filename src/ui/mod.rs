pub mod area_chart;
pub mod bottom_strip;
pub mod candlestick;
pub mod chart;
pub mod horizon_chart;
pub mod cpu;
pub mod footer;
pub mod gpu;
pub mod header;
pub mod about;
pub mod help;
pub mod update;
pub mod memory;
pub mod network_disk;
pub mod process_table;
pub mod sparkline_buf;
pub mod status;
pub mod text;
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
        help::render(f, app.telemetry_enabled);
    }
    if app.show_about {
        about::render(f);
    }
    if app.show_update {
        update::render(f, app);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CategoryFilter, ChartTab, PanelFocus, SortColumn};
    use crate::config::Config;
    use crate::data::DataSnapshot;
    use crate::data::lhm::{CpuSensors, GpuSensors, MemorySensors};
    use crate::data::network::{InterfaceStats, NetworkStats};
    use crate::data::process::{AiState, ProcessCategory, ProcessInfo};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    fn test_app() -> App {
        let config: Config = toml::from_str("").expect("empty config uses all defaults");
        App::new(
            config,
            dofek::telemetry::TelemetryHandle::disabled(),
            Arc::new(AtomicU64::new(500)),
        )
    }

    fn proc(pid: u32, name: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cpu_percent: 12.5,
            memory_bytes: 1024 * 1024 * 512,
            vram_bytes: Some(1024 * 1024 * 1024),
            is_ai_workload: true,
            ai_state: AiState::Inferring,
            category: ProcessCategory::Ai,
            plugin_label: Some("mock".into()),
        }
    }

    /// A snapshot with every named panel populated, so each renderer has real
    /// data to lay out rather than bailing on an empty state.
    fn snapshot(process_names: &[&str]) -> DataSnapshot {
        DataSnapshot {
            cpu: CpuSensors {
                name: "Test CPU".into(),
                total_load: 42.0,
                per_core_load: vec![10.0, 20.0, 30.0, 40.0],
                temperature: Some(55.0),
                power: Some(25.0),
            },
            memory: MemorySensors {
                used_percent: 60.0,
                used_gb: 9.6,
                total_gb: 16.0,
                swap_used_percent: 5.0,
            },
            gpus: vec![GpuSensors {
                name: "Test GPU".into(),
                utilization: 70.0,
                vram_used_mb: 4096.0,
                vram_total_mb: 8192.0,
                temperature: 65.0,
                power_watts: 120.0,
            }],
            network: NetworkStats {
                interfaces: vec![InterfaceStats {
                    name: "en0".into(),
                    rx_bytes_per_sec: 1_500_000.0,
                    tx_bytes_per_sec: 250_000.0,
                }],
            },
            processes: process_names
                .iter()
                .enumerate()
                .map(|(i, n)| proc(i as u32 + 1, n))
                .collect(),
            ..Default::default()
        }
    }

    /// Render every view at `size`, asserting only that nothing panics.
    fn render_all_views(app: &mut App, (w, h): (u16, u16)) {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();

        for focus in [PanelFocus::Dashboard, PanelFocus::Processes] {
            app.focus = focus;
            for tab in [
                ChartTab::Cpu,
                ChartTab::Gpu,
                ChartTab::Mem,
                ChartTab::Net,
                ChartTab::Disk,
            ] {
                app.chart_tab = tab;
                for filter in [
                    CategoryFilter::All,
                    CategoryFilter::Ai,
                    CategoryFilter::Dev,
                    CategoryFilter::Watch,
                ] {
                    app.category_filter = filter;
                    term.draw(|f| render(f, app)).unwrap();
                }
            }
        }
    }

    /// Terminal widths that put the truncation boundary in different places,
    /// including sizes narrower than the labels being truncated.
    const SIZES: &[(u16, u16)] = &[
        (200, 60),
        (120, 40),
        (80, 24),
        (60, 20),
        (40, 15),
        (24, 10),
        (12, 8),
    ];

    #[test]
    fn renders_ascii_processes_at_every_size() {
        let mut app = test_app();
        app.update_data(snapshot(&["ollama", "chrome", "cargo", "chrome"]));
        for size in SIZES {
            render_all_views(&mut app, *size);
        }
    }

    /// The regression behind the shared `truncate`: a multibyte process name
    /// used to byte-slice mid-codepoint and panic the whole TUI. Names here
    /// span every character length around the column budgets, so at least one
    /// lands exactly on a boundary at every terminal width above.
    #[test]
    fn renders_multibyte_process_names_without_panicking() {
        let cjk = "字幕処理サービス管理常駐監視";
        let emoji = "🔥🚀🎉🧠💾🖥️🌊🍕🎯🔮";
        let mixed = "py字torch🔥サーバ";

        let mut names: Vec<String> = Vec::new();
        for n in 1..=cjk.chars().count() {
            names.push(cjk.chars().take(n).collect());
        }
        for n in 1..=emoji.chars().count() {
            names.push(emoji.chars().take(n).collect());
        }
        for n in 1..=mixed.chars().count() {
            names.push(mixed.chars().take(n).collect());
        }
        // Combining marks and RTL, which also break naive byte slicing.
        names.push("café-server".into());
        names.push("שרת-בינה".into());

        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut app = test_app();
        app.update_data(snapshot(&refs));

        for size in SIZES {
            render_all_views(&mut app, *size);
        }
    }

    /// Multibyte text also reaches the CPU/GPU/interface title cells, which
    /// truncate against the panel width rather than a fixed column.
    #[test]
    fn renders_multibyte_device_names_without_panicking() {
        let mut app = test_app();
        let mut snap = snapshot(&["ollama"]);
        snap.cpu.name = "インテル製プロセッサ第14世代".into();
        snap.gpus[0].name = "🎮グラフィックス処理装置🎮".into();
        snap.network.interfaces[0].name = "イーサネット接続".into();
        app.update_data(snap);

        for size in SIZES {
            render_all_views(&mut app, *size);
        }
    }

    #[test]
    fn renders_grouped_view_and_overlays() {
        let mut app = test_app();
        app.update_data(snapshot(&["字幕処理", "字幕処理", "字幕処理", "🔥"]));
        app.grouped_view = true;
        app.expanded_groups.insert("字幕処理".to_string());
        app.selected_process = Some(0);
        app.sort_column = SortColumn::Cpu;

        for size in SIZES {
            render_all_views(&mut app, *size);
        }

        // Overlays draw on top of the dashboard and have their own layout math.
        for (flag_name, size) in [("help", (100, 30)), ("about", (100, 30))] {
            app.show_help = flag_name == "help";
            app.show_about = flag_name == "about";
            render_all_views(&mut app, size);
        }
    }

    #[test]
    fn renders_with_no_data_at_all() {
        let mut app = test_app();
        for size in SIZES {
            render_all_views(&mut app, *size);
        }
    }
}
