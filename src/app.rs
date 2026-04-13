use crossterm::event::{KeyCode, KeyEvent};

use crate::config::Config;
use crate::data::DataSnapshot;
use crate::ui::sparkline_buf::{CandleBuf, SparklineBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelFocus {
    Dashboard,
    Processes,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChartTab {
    Cpu,
    Gpu,
    Mem,
    Net,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CategoryFilter {
    All,
    Ai,
    Dev,
    Watch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuTab {
    All,
    Device(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortColumn {
    Name,
    Pid,
    Cpu,
    Memory,
    Vram,
}

impl SortColumn {
    pub fn next(self) -> Self {
        match self {
            SortColumn::Name => SortColumn::Pid,
            SortColumn::Pid => SortColumn::Cpu,
            SortColumn::Cpu => SortColumn::Memory,
            SortColumn::Memory => SortColumn::Vram,
            SortColumn::Vram => SortColumn::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortColumn::Name => "NAME",
            SortColumn::Pid => "PID",
            SortColumn::Cpu => "CPU%",
            SortColumn::Memory => "MEM",
            SortColumn::Vram => "VRAM",
        }
    }
}

pub struct HistoryBuffers {
    pub cpu_total: SparklineBuf,
    pub cpu_candle: CandleBuf,
    pub memory_used: SparklineBuf,
    pub gpu_util: SparklineBuf,
    pub gpu_vram: SparklineBuf,
    /// Per-GPU utilization history (indexed by GPU device order).
    pub gpu_util_per_device: Vec<SparklineBuf>,
    pub net_rx: SparklineBuf,
    pub net_tx: SparklineBuf,
}

impl HistoryBuffers {
    pub fn new(capacity: usize) -> Self {
        // 10 samples per candle: at 500ms refresh = 5s per candle
        let samples_per_candle = 10;
        Self {
            cpu_total: SparklineBuf::new(capacity),
            cpu_candle: CandleBuf::new(capacity, samples_per_candle),
            memory_used: SparklineBuf::new(capacity),
            gpu_util: SparklineBuf::new(capacity),
            gpu_vram: SparklineBuf::new(capacity),
            gpu_util_per_device: Vec::new(),
            net_rx: SparklineBuf::new(capacity),
            net_tx: SparklineBuf::new(capacity),
        }
    }
}

pub struct App {
    pub data: DataSnapshot,
    pub history: HistoryBuffers,
    pub config: Config,
    pub focus: PanelFocus,
    pub chart_tab: ChartTab,
    pub category_filter: CategoryFilter,
    pub gpu_tab: GpuTab,
    pub sort_column: SortColumn,
    pub sort_ascending: bool,
    pub show_help: bool,
    pub should_quit: bool,
    pub refresh_ms: u64,
    pub selected_process: Option<usize>,
    /// Chart/watchlist horizontal split percentage (chart gets this %, watchlist gets the rest).
    pub split_pct: u16,
}

impl App {
    pub fn new(config: Config) -> Self {
        let history_len = config.general.history_len;
        let refresh_ms = config.general.refresh_ms;
        Self {
            data: DataSnapshot::default(),
            history: HistoryBuffers::new(history_len),
            config,
            focus: PanelFocus::Dashboard,
            chart_tab: ChartTab::Cpu,
            category_filter: CategoryFilter::All,
            gpu_tab: GpuTab::All,
            sort_column: SortColumn::Memory,
            sort_ascending: false,
            show_help: false,
            should_quit: false,
            refresh_ms,
            selected_process: None,
            split_pct: 58,
        }
    }

    /// Returns the primary (first) GPU, if any.
    pub fn primary_gpu(&self) -> Option<&crate::data::lhm::GpuSensors> {
        self.data.gpus.first()
    }

    pub fn update_data(&mut self, snapshot: DataSnapshot) {
        let history_len = self.config.general.history_len;

        // Update sparkline history
        self.history.cpu_total.push_percent(snapshot.cpu.total_load);
        self.history.cpu_candle.push(snapshot.cpu.total_load);
        self.history.memory_used.push_percent(snapshot.memory.used_percent);

        // Aggregate GPU history (first GPU for backward compat)
        if let Some(gpu) = snapshot.gpus.first() {
            self.history.gpu_util.push_percent(gpu.utilization);
            let vram_pct = if gpu.vram_total_mb > 0.0 {
                gpu.vram_used_mb / gpu.vram_total_mb * 100.0
            } else {
                0.0
            };
            self.history.gpu_vram.push_percent(vram_pct);
        }

        // Per-GPU history: grow vector if new GPUs appear
        while self.history.gpu_util_per_device.len() < snapshot.gpus.len() {
            self.history.gpu_util_per_device.push(SparklineBuf::new(history_len));
        }
        for (i, gpu) in snapshot.gpus.iter().enumerate() {
            self.history.gpu_util_per_device[i].push_percent(gpu.utilization);
        }

        // Network: push bytes/sec (use raw values scaled to KB/s)
        if let Some(iface) = snapshot.network.interfaces.first() {
            self.history.net_rx.push_raw((iface.rx_bytes_per_sec / 1024.0) as u64);
            self.history.net_tx.push_raw((iface.tx_bytes_per_sec / 1024.0) as u64);
        }

        self.data = snapshot;

        // Sort processes
        self.sort_processes();
    }

    fn sort_processes(&mut self) {
        let asc = self.sort_ascending;
        self.data.processes.sort_by(|a, b| {
            let cmp = match self.sort_column {
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Cpu => a.cpu_percent.partial_cmp(&b.cpu_percent).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Memory => a.memory_bytes.cmp(&b.memory_bytes),
                SortColumn::Vram => {
                    let va = a.vram_bytes.unwrap_or(0);
                    let vb = b.vram_bytes.unwrap_or(0);
                    va.cmp(&vb)
                }
            };
            if asc { cmp } else { cmp.reverse() }
        });
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.show_help {
            self.show_help = false;
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Tab => {
                self.sort_column = self.sort_column.next();
                self.sort_processes();
            }
            // Chart tab switching
            KeyCode::Char('c') => self.chart_tab = ChartTab::Cpu,
            KeyCode::Char('g') => self.chart_tab = ChartTab::Gpu,
            KeyCode::Char('m') => self.chart_tab = ChartTab::Mem,
            KeyCode::Char('n') => self.chart_tab = ChartTab::Net,
            // Full-screen process view
            KeyCode::Char('p') => self.focus = PanelFocus::Processes,
            KeyCode::Esc => self.focus = PanelFocus::Dashboard,
            // Category filter
            KeyCode::Char('1') => self.category_filter = CategoryFilter::All,
            KeyCode::Char('2') => self.category_filter = CategoryFilter::Ai,
            KeyCode::Char('3') => self.category_filter = CategoryFilter::Dev,
            KeyCode::Char('4') => self.category_filter = CategoryFilter::Watch,
            // Refresh rate
            KeyCode::Char('+') | KeyCode::Char('=') => {
                if self.refresh_ms > 100 {
                    self.refresh_ms = self.refresh_ms.saturating_sub(100);
                }
            }
            KeyCode::Char('-') => {
                self.refresh_ms = (self.refresh_ms + 100).min(5000);
            }
            KeyCode::Char('s') => {
                self.save_snapshot();
            }
            // Resize chart/watchlist split
            KeyCode::Char('[') => {
                self.split_pct = self.split_pct.saturating_sub(5).max(25);
            }
            KeyCode::Char(']') => {
                self.split_pct = (self.split_pct + 5).min(85);
            }
            _ => {}
        }
    }

    fn save_snapshot(&self) {
        let timestamp = chrono_lite_timestamp();
        let dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("dofek-snapshots");
        let _ = std::fs::create_dir_all(&dir);
        let filename = dir.join(format!("dofek-snapshot-{timestamp}.txt"));
        let content = format!(
            "dofek snapshot — {timestamp}\n\
             \n\
             CPU: {} — {:.1}%\n\
             Memory: {:.1} / {:.1} GB ({:.1}%)\n\
             GPU: {}\n\
             Processes: {}\n",
            self.data.cpu.name,
            self.data.cpu.total_load,
            self.data.memory.used_gb,
            self.data.memory.total_gb,
            self.data.memory.used_percent,
            self.primary_gpu().map(|g| format!(
                "{} — {:.1}% util, {:.0}/{:.0} MB VRAM, {:.0}°C",
                g.name, g.utilization, g.vram_used_mb, g.vram_total_mb, g.temperature
            )).unwrap_or_else(|| "N/A".to_string()),
            self.data.processes.len(),
        );
        let _ = std::fs::write(&filename, content);
        log::info!("Snapshot saved to {}", filename.display());
    }
}

fn chrono_lite_timestamp() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}
