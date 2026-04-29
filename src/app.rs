use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

use crossterm::event::{KeyCode, KeyEvent};

use crate::config::Config;
use crate::data::DataSnapshot;
use crate::data::process::{ProcessCategory, ProcessInfo};
use crate::ui::sparkline_buf::{CandleBuf, SparklineBuf};

use dofek::settings::UserSettings;
use dofek::telemetry::{TelemetryEvent, TelemetryHandle};
use dofek::update::UpdateInfo;

/// Result of an update check, shared between the worker thread and the UI.
#[derive(Debug, Clone, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    Ready(UpdateInfo),
    Error(String),
}

/// Pending kill-confirmation state.
pub enum ConfirmKill {
    /// Kill a single process.
    Single { pid: u32, name: String },
    /// Kill all processes matching the current search/filter.
    Batch { targets: Vec<(u32, String)> },
}

/// A row in the grouped process view — either a group header or a child process.
#[derive(Debug, Clone)]
pub enum ProcessRow<'a> {
    /// Collapsed or expandable group header with aggregate stats.
    Group {
        name: String,
        count: usize,
        cpu_total: f32,
        mem_total: u64,
        vram_total: u64,
        pids: Vec<u32>,
        expanded: bool,
        category: ProcessCategory,
    },
    /// Individual process (child of an expanded group, or singleton).
    Process(&'a ProcessInfo),
}

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
    Disk,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CategoryFilter {
    All,
    Ai,
    Dev,
    Watch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChartMode {
    Default,  // Candle for CPU, Area for GPU/MEM/NET
    Horizon,
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
    pub disk_read: SparklineBuf,
    pub disk_write: SparklineBuf,
}

impl HistoryBuffers {
    pub fn new(capacity: usize) -> Self {
        // 1 sample per candle with synthetic spread (matches GUI behavior)
        let samples_per_candle = 1;
        Self {
            cpu_total: SparklineBuf::new(capacity),
            cpu_candle: CandleBuf::new(capacity, samples_per_candle),
            memory_used: SparklineBuf::new(capacity),
            gpu_util: SparklineBuf::new(capacity),
            gpu_vram: SparklineBuf::new(capacity),
            gpu_util_per_device: Vec::new(),
            net_rx: SparklineBuf::new(capacity),
            net_tx: SparklineBuf::new(capacity),
            disk_read: SparklineBuf::new(capacity),
            disk_write: SparklineBuf::new(capacity),
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
    pub chart_mode: ChartMode,
    pub show_help: bool,
    pub show_about: bool,
    pub show_update: bool,
    pub update_state: Arc<Mutex<UpdateState>>,
    pub should_quit: bool,
    /// Polling interval shared with the data collector thread. The TUI's `+`/`-`
    /// keys mutate this in place so the collector picks up the new cadence on
    /// its next iteration without needing to be respawned.
    pub refresh_ms: Arc<AtomicU64>,
    pub selected_process: Option<usize>,
    /// Scroll offset for the full-screen process table.
    pub process_scroll: usize,
    /// Live search query for process name filtering.
    pub search_query: String,
    /// Whether the search input is actively receiving keystrokes.
    pub search_active: bool,
    /// Whether the grouped (tree) process view is active.
    pub grouped_view: bool,
    /// Set of group names that are currently expanded.
    pub expanded_groups: HashSet<String>,
    /// Pending kill confirmation (shown as overlay in process view).
    pub confirm_kill: Option<ConfirmKill>,
    /// Brief status message after a kill attempt (cleared on next key).
    pub kill_status: Option<String>,
    /// Chart/watchlist horizontal split percentage (chart gets this %, watchlist gets the rest).
    pub split_pct: u16,
    pub telemetry: TelemetryHandle,
    pub telemetry_enabled: bool,
}

impl App {
    pub fn new(config: Config, telemetry: TelemetryHandle, refresh_ms: Arc<AtomicU64>) -> Self {
        let history_len = config.general.history_len;
        Self {
            data: DataSnapshot::default(),
            history: HistoryBuffers::new(history_len),
            config,
            focus: PanelFocus::Dashboard,
            chart_tab: ChartTab::Cpu,
            category_filter: CategoryFilter::All,
            gpu_tab: GpuTab::All,
            chart_mode: ChartMode::Default,
            sort_column: SortColumn::Memory,
            sort_ascending: false,
            show_help: false,
            show_about: false,
            show_update: false,
            update_state: Arc::new(Mutex::new(UpdateState::Idle)),
            should_quit: false,
            refresh_ms,
            selected_process: None,
            process_scroll: 0,
            search_query: String::new(),
            search_active: false,
            grouped_view: false,
            expanded_groups: HashSet::new(),
            confirm_kill: None,
            kill_status: None,
            split_pct: 58,
            telemetry_enabled: false,
            telemetry,
        }
    }

    /// Returns the primary (first) GPU, if any.
    pub fn primary_gpu(&self) -> Option<&crate::data::lhm::GpuSensors> {
        self.data.gpus.first()
    }

    pub fn update_data(&mut self, snapshot: DataSnapshot) {
        let history_len = self.config.general.history_len;

        // Update sparkline history
        // Skip first bogus sysinfo sample (always reports ~100% before a delta is computed)
        let skip_first_cpu = self.history.cpu_total.len() == 0 && snapshot.cpu.total_load >= 99.0;
        if !skip_first_cpu {
            self.history.cpu_total.push_percent(snapshot.cpu.total_load);
            self.history.cpu_candle.push(snapshot.cpu.total_load);
        }
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

        // Disk: push aggregate bytes/sec scaled to KB/s
        self.history.disk_read.push_raw((snapshot.disk.total_read_bytes_per_sec / 1024.0) as u64);
        self.history.disk_write.push_raw((snapshot.disk.total_write_bytes_per_sec / 1024.0) as u64);

        self.data = snapshot;

        // Sort processes
        self.sort_processes();

        // Clamp selection to valid range
        if let Some(sel) = self.selected_process {
            let count = self.visible_row_count();
            if count > 0 {
                self.selected_process = Some(sel.min(count - 1));
            } else {
                self.selected_process = None;
            }
        }
    }

    fn sort_processes(&mut self) {
        let asc = self.sort_ascending;
        self.data.processes.sort_by(|a, b| {
            let cmp = match self.sort_column {
                SortColumn::Name => a.name.as_bytes().iter()
                    .map(|c| c.to_ascii_lowercase())
                    .cmp(b.name.as_bytes().iter().map(|c| c.to_ascii_lowercase())),
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

    #[allow(clippy::collapsible_match, clippy::collapsible_if)]
    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.show_help {
            if key.code == KeyCode::Char('T') {
                self.telemetry_enabled = !self.telemetry_enabled;
                return;
            }
            self.show_help = false;
            return;
        }
        if self.show_about {
            self.show_about = false;
            return;
        }
        if self.show_update {
            self.show_update = false;
            return;
        }

        // Kill confirmation dialog intercepts all keys
        if self.confirm_kill.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(ck) = self.confirm_kill.take() {
                        match ck {
                            ConfirmKill::Single { pid, name } => {
                                self.execute_kill(pid, &name);
                            }
                            ConfirmKill::Batch { targets } => {
                                self.execute_kill_batch(&targets);
                            }
                        }
                    }
                }
                _ => {
                    self.confirm_kill = None;
                }
            }
            return;
        }

        // Clear kill status on any key
        if self.kill_status.is_some() {
            self.kill_status = None;
        }

        // Search input mode: most keys go to the search buffer
        if self.search_active {
            match key.code {
                KeyCode::Esc => {
                    self.search_query.clear();
                    self.search_active = false;
                    self.reset_selection();
                }
                KeyCode::Enter => {
                    // Lock filter, exit input mode (query stays)
                    self.search_active = false;
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.reset_selection();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.reset_selection();
                }
                // Navigation still works during search
                KeyCode::Up => { self.move_selection(-1); }
                KeyCode::Down => { self.move_selection(1); }
                KeyCode::Delete => { self.initiate_kill(); }
                _ => {}
            }
            return;
        }

        // Process view: handle navigation and kill keys before global keys
        if self.focus == PanelFocus::Processes {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.move_selection(-1);
                    return;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.move_selection(1);
                    return;
                }
                KeyCode::Home => {
                    self.selected_process = Some(0);
                    self.process_scroll = 0;
                    return;
                }
                KeyCode::End => {
                    let count = self.visible_row_count();
                    if count > 0 {
                        self.selected_process = Some(count - 1);
                    }
                    return;
                }
                KeyCode::Delete | KeyCode::Char('x') => {
                    self.initiate_kill();
                    return;
                }
                KeyCode::Char('X') => {
                    self.initiate_kill_all();
                    return;
                }
                KeyCode::Char('t') => {
                    self.grouped_view = !self.grouped_view;
                    self.reset_selection();
                    return;
                }
                KeyCode::Right | KeyCode::Enter => {
                    if self.grouped_view {
                        self.toggle_group_expand(true);
                    }
                    return;
                }
                KeyCode::Left => {
                    if self.grouped_view {
                        self.toggle_group_expand(false);
                    }
                    return;
                }
                KeyCode::Char('/') => {
                    self.search_active = true;
                    // Don't clear existing query — let user refine
                    return;
                }
                KeyCode::Esc => {
                    if !self.search_query.is_empty() {
                        // First Esc clears search, second goes back to dashboard
                        self.search_query.clear();
                        self.reset_selection();
                        return;
                    }
                    // Fall through to global Esc handler below
                }
                _ => {} // fall through to global keys
            }
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('a') => self.show_about = true,
            KeyCode::Char('u') => {
                self.show_update = true;
                self.trigger_update_check();
            }
            KeyCode::Tab => {
                self.sort_column = self.sort_column.next();
                self.sort_processes();
            }
            // Chart tab switching
            KeyCode::Char('c') => {
                self.chart_tab = ChartTab::Cpu;
                self.telemetry.track(TelemetryEvent::TabSwitch { tab: "cpu".into() });
            }
            KeyCode::Char('g') => {
                self.chart_tab = ChartTab::Gpu;
                self.telemetry.track(TelemetryEvent::TabSwitch { tab: "gpu".into() });
            }
            KeyCode::Char('m') => {
                self.chart_tab = ChartTab::Mem;
                self.telemetry.track(TelemetryEvent::TabSwitch { tab: "mem".into() });
            }
            KeyCode::Char('n') => {
                self.chart_tab = ChartTab::Net;
                self.telemetry.track(TelemetryEvent::TabSwitch { tab: "net".into() });
            }
            KeyCode::Char('d') => {
                self.chart_tab = ChartTab::Disk;
                self.telemetry.track(TelemetryEvent::TabSwitch { tab: "disk".into() });
            }
            // Full-screen process view
            KeyCode::Char('p') => {
                self.focus = PanelFocus::Processes;
                if self.selected_process.is_none() && !self.data.processes.is_empty() {
                    self.selected_process = Some(0);
                }
                self.telemetry.track(TelemetryEvent::PanelSwitch { panel: "processes".into() });
            }
            KeyCode::Esc => {
                self.focus = PanelFocus::Dashboard;
                self.telemetry.track(TelemetryEvent::PanelSwitch { panel: "dashboard".into() });
            }
            // Category filter (reset selection since filtered list changes)
            KeyCode::Char('1') => {
                self.category_filter = CategoryFilter::All;
                self.reset_selection();
                self.telemetry.track(TelemetryEvent::FilterChange { filter: "all".into() });
            }
            KeyCode::Char('2') => {
                self.category_filter = CategoryFilter::Ai;
                self.reset_selection();
                self.telemetry.track(TelemetryEvent::FilterChange { filter: "ai".into() });
            }
            KeyCode::Char('3') => {
                self.category_filter = CategoryFilter::Dev;
                self.reset_selection();
                self.telemetry.track(TelemetryEvent::FilterChange { filter: "dev".into() });
            }
            KeyCode::Char('4') => {
                self.category_filter = CategoryFilter::Watch;
                self.reset_selection();
                self.telemetry.track(TelemetryEvent::FilterChange { filter: "watch".into() });
            }
            // Refresh rate. Mutating the Arc here propagates to the collector
            // thread (next sleep) and to every UI panel that reads it.
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let cur = self.refresh_ms.load(Ordering::Relaxed);
                if cur > 100 {
                    self.refresh_ms.store(cur.saturating_sub(100), Ordering::Relaxed);
                }
            }
            KeyCode::Char('-') => {
                let cur = self.refresh_ms.load(Ordering::Relaxed);
                self.refresh_ms.store((cur + 100).min(5000), Ordering::Relaxed);
            }
            KeyCode::Char('s') => {
                self.save_snapshot();
            }
            // Toggle horizon chart mode
            KeyCode::Char('h') => {
                self.chart_mode = match self.chart_mode {
                    ChartMode::Default => ChartMode::Horizon,
                    ChartMode::Horizon => ChartMode::Default,
                };
                let mode = match self.chart_mode {
                    ChartMode::Default => "default",
                    ChartMode::Horizon => "horizon",
                };
                self.telemetry.track(TelemetryEvent::ChartModeToggle { mode: mode.into() });
            }
            // Resize chart/watchlist split
            KeyCode::Char('[') => {
                self.split_pct = self.split_pct.saturating_sub(5).max(30);
            }
            KeyCode::Char(']') => {
                self.split_pct = (self.split_pct + 5).min(70);
            }
            _ => {}
        }
    }

    fn reset_selection(&mut self) {
        self.selected_process = if self.focus == PanelFocus::Processes { Some(0) } else { None };
        self.process_scroll = 0;
    }

    /// Returns processes filtered by category and search query.
    pub fn filtered_processes(&self) -> Vec<&ProcessInfo> {
        let query = self.search_query.to_ascii_lowercase();
        self.data.processes.iter()
            .filter(|p| match self.category_filter {
                CategoryFilter::All => true,
                CategoryFilter::Ai => p.category == ProcessCategory::Ai,
                CategoryFilter::Dev => p.category == ProcessCategory::Dev,
                CategoryFilter::Watch => p.category == ProcessCategory::Watch,
            })
            .filter(|p| {
                query.is_empty() || p.name.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    /// Build the grouped process row list for the tree view.
    pub fn grouped_rows(&self) -> Vec<ProcessRow<'_>> {
        let filtered = self.filtered_processes();
        let mut groups: Vec<(String, Vec<&ProcessInfo>)> = Vec::new();
        let mut group_map: HashMap<String, usize> = HashMap::new();

        for p in &filtered {
            if let Some(&idx) = group_map.get(&p.name) {
                groups[idx].1.push(p);
            } else {
                group_map.insert(p.name.clone(), groups.len());
                groups.push((p.name.clone(), vec![p]));
            }
        }

        // Sort groups by the same column as processes (using aggregate values)
        let asc = self.sort_ascending;
        groups.sort_by(|a, b| {
            let cmp = match self.sort_column {
                SortColumn::Name => a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()),
                SortColumn::Pid => a.1[0].pid.cmp(&b.1[0].pid),
                SortColumn::Cpu => {
                    let sa: f32 = a.1.iter().map(|p| p.cpu_percent).sum();
                    let sb: f32 = b.1.iter().map(|p| p.cpu_percent).sum();
                    sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                }
                SortColumn::Memory => {
                    let sa: u64 = a.1.iter().map(|p| p.memory_bytes).sum();
                    let sb: u64 = b.1.iter().map(|p| p.memory_bytes).sum();
                    sa.cmp(&sb)
                }
                SortColumn::Vram => {
                    let sa: u64 = a.1.iter().map(|p| p.vram_bytes.unwrap_or(0)).sum();
                    let sb: u64 = b.1.iter().map(|p| p.vram_bytes.unwrap_or(0)).sum();
                    sa.cmp(&sb)
                }
            };
            if asc { cmp } else { cmp.reverse() }
        });

        let mut rows = Vec::new();
        for (name, procs) in &groups {
            let expanded = self.expanded_groups.contains(name);
            let count = procs.len();

            if count == 1 {
                // Singleton — render as a plain process row
                rows.push(ProcessRow::Process(procs[0]));
            } else {
                // Group header
                let cpu_total: f32 = procs.iter().map(|p| p.cpu_percent).sum();
                let mem_total: u64 = procs.iter().map(|p| p.memory_bytes).sum();
                let vram_total: u64 = procs.iter().map(|p| p.vram_bytes.unwrap_or(0)).sum();
                let pids: Vec<u32> = procs.iter().map(|p| p.pid).collect();
                // Use the highest-priority category from the group
                let category = procs.iter().map(|p| p.category).min_by_key(|c| match c {
                    ProcessCategory::Watch => 0,
                    ProcessCategory::Ai => 1,
                    ProcessCategory::Dev => 2,
                    ProcessCategory::None => 3,
                }).unwrap_or(ProcessCategory::None);

                rows.push(ProcessRow::Group {
                    name: name.clone(),
                    count,
                    cpu_total,
                    mem_total,
                    vram_total,
                    pids,
                    expanded,
                    category,
                });

                if expanded {
                    for p in procs {
                        rows.push(ProcessRow::Process(p));
                    }
                }
            }
        }
        rows
    }

    fn toggle_group_expand(&mut self, expand: bool) {
        let rows = self.grouped_rows();
        let idx = self.selected_process.unwrap_or(0);
        // Extract the info we need before dropping the borrow
        let action: Option<(String, bool, Option<usize>)> = match rows.get(idx) {
            Some(ProcessRow::Group { name, expanded, .. }) => {
                if expand && !expanded {
                    Some((name.clone(), true, None))
                } else if !expand && *expanded {
                    Some((name.clone(), false, None))
                } else {
                    None
                }
            }
            Some(ProcessRow::Process(_)) if !expand => {
                // Find parent group above
                let mut found = None;
                for i in (0..idx).rev() {
                    if let Some(ProcessRow::Group { name, .. }) = rows.get(i) {
                        found = Some((name.clone(), false, Some(i)));
                        break;
                    }
                }
                found
            }
            _ => None,
        };
        drop(rows);

        if let Some((name, insert, sel)) = action {
            if insert {
                self.expanded_groups.insert(name);
            } else {
                self.expanded_groups.remove(&name);
            }
            if let Some(s) = sel {
                self.selected_process = Some(s);
            }
        }
    }

    /// Returns the number of visible rows (respects grouped/flat view).
    fn visible_row_count(&self) -> usize {
        if self.grouped_view {
            self.grouped_rows().len()
        } else {
            self.filtered_processes().len()
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let count = self.visible_row_count();
        if count == 0 {
            return;
        }
        let current = self.selected_process.unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, count as i32 - 1) as usize;
        self.selected_process = Some(next);
    }

    fn initiate_kill(&mut self) {
        let idx = match self.selected_process {
            Some(i) => i,
            None => return,
        };

        if self.grouped_view {
            let rows = self.grouped_rows();
            match rows.get(idx) {
                Some(ProcessRow::Process(p)) => {
                    self.confirm_kill = Some(ConfirmKill::Single {
                        pid: p.pid,
                        name: p.name.clone(),
                    });
                }
                Some(ProcessRow::Group { name, pids, .. }) => {
                    let targets: Vec<(u32, String)> = pids.iter().map(|&pid| (pid, name.clone())).collect();
                    self.confirm_kill = Some(ConfirmKill::Batch { targets });
                }
                None => {}
            }
        } else {
            let filtered = self.filtered_processes();
            if let Some(proc) = filtered.get(idx) {
                self.confirm_kill = Some(ConfirmKill::Single {
                    pid: proc.pid,
                    name: proc.name.clone(),
                });
            }
        }
    }

    fn initiate_kill_all(&mut self) {
        let filtered = self.filtered_processes();
        if filtered.is_empty() {
            return;
        }
        let targets: Vec<(u32, String)> = filtered.iter()
            .map(|p| (p.pid, p.name.clone()))
            .collect();
        self.confirm_kill = Some(ConfirmKill::Batch { targets });
    }

    fn execute_kill(&mut self, pid: u32, name: &str) {
        match kill_process_by_pid(pid) {
            Ok(()) => {
                self.kill_status = Some(format!("Killed {} (PID {})", name, pid));
                self.telemetry.track(TelemetryEvent::ProcessKill {
                    success: true,
                });
            }
            Err(e) => {
                self.kill_status = Some(format!("Failed to kill {}: {}", name, e));
                self.telemetry.track(TelemetryEvent::ProcessKill {
                    success: false,
                });
            }
        }
    }

    fn execute_kill_batch(&mut self, targets: &[(u32, String)]) {
        let mut killed = 0usize;
        let mut failed = 0usize;
        for (pid, _name) in targets {
            match kill_process_by_pid(*pid) {
                Ok(()) => killed += 1,
                Err(_) => failed += 1,
            }
        }
        let total = targets.len();
        if failed == 0 {
            self.kill_status = Some(format!("Killed all {total} processes"));
        } else {
            self.kill_status = Some(format!("Killed {killed}/{total} ({failed} failed)"));
        }
        self.telemetry.track(TelemetryEvent::ProcessKill {
            success: failed == 0,
        });
    }

    pub fn apply_settings(&mut self, s: &UserSettings) {
        self.chart_tab = match s.chart_tab.as_str() {
            "gpu" => ChartTab::Gpu,
            "mem" => ChartTab::Mem,
            "net" => ChartTab::Net,
            "disk" => ChartTab::Disk,
            _ => ChartTab::Cpu,
        };
        self.chart_mode = match s.chart_mode.as_str() {
            "horizon" => ChartMode::Horizon,
            _ => ChartMode::Default,
        };
        self.sort_column = match s.sort_column.as_str() {
            "name" => SortColumn::Name,
            "pid" => SortColumn::Pid,
            "cpu" => SortColumn::Cpu,
            "vram" => SortColumn::Vram,
            _ => SortColumn::Memory,
        };
        self.sort_ascending = s.sort_ascending;
        self.category_filter = match s.category_filter.as_str() {
            "ai" => CategoryFilter::Ai,
            "dev" => CategoryFilter::Dev,
            "watch" => CategoryFilter::Watch,
            _ => CategoryFilter::All,
        };
        self.split_pct = s.split_pct.clamp(30, 70);
        // Intentionally do NOT load refresh_ms from settings.toml — dofek.toml
        // is the source of truth for the polling interval. settings.toml stores
        // a default value that would otherwise silently override the user's
        // explicit dofek.toml choice.
        self.telemetry_enabled = s.telemetry_enabled;
    }

    pub fn to_settings(&self, prev: &UserSettings) -> UserSettings {
        UserSettings {
            chart_tab: match self.chart_tab {
                ChartTab::Cpu => "cpu",
                ChartTab::Gpu => "gpu",
                ChartTab::Mem => "mem",
                ChartTab::Net => "net",
                ChartTab::Disk => "disk",
            }.to_string(),
            chart_mode: match self.chart_mode {
                ChartMode::Default => "default",
                ChartMode::Horizon => "horizon",
            }.to_string(),
            sort_column: match self.sort_column {
                SortColumn::Name => "name",
                SortColumn::Pid => "pid",
                SortColumn::Cpu => "cpu",
                SortColumn::Memory => "memory",
                SortColumn::Vram => "vram",
            }.to_string(),
            sort_ascending: self.sort_ascending,
            category_filter: match self.category_filter {
                CategoryFilter::All => "all",
                CategoryFilter::Ai => "ai",
                CategoryFilter::Dev => "dev",
                CategoryFilter::Watch => "watch",
            }.to_string(),
            split_pct: self.split_pct,
            refresh_ms: self.refresh_ms.load(Ordering::Relaxed),
            anonymous_id: prev.anonymous_id.clone(),
            telemetry_prompted: prev.telemetry_prompted,
            telemetry_enabled: self.telemetry_enabled,
            enable_tray: prev.enable_tray,
            close_to_tray: prev.close_to_tray,
            start_in_tray: prev.start_in_tray,
            tray_show_text: prev.tray_show_text,
            tray_display_mode: prev.tray_display_mode.clone(),
            check_updates_on_startup: prev.check_updates_on_startup,
        }
    }

    /// Spawn a worker thread that hits the GitHub Releases API and writes the
    /// outcome into `update_state`. Cheap to call: skips the spawn if a check
    /// is already in flight.
    pub fn trigger_update_check(&self) {
        {
            let mut s = self.update_state.lock().unwrap();
            if matches!(*s, UpdateState::Checking) {
                return;
            }
            *s = UpdateState::Checking;
        }
        let slot = Arc::clone(&self.update_state);
        std::thread::spawn(move || {
            let next = match dofek::update::check() {
                Ok(info) => UpdateState::Ready(info),
                Err(e) => UpdateState::Error(e.to_string()),
            };
            *slot.lock().unwrap() = next;
        });
    }

    fn save_snapshot(&self) {
        let timestamp = chrono_lite_timestamp();
        let dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("dofek-snapshots");
        let _ = std::fs::create_dir_all(&dir);
        let filename = dir.join(format!("dofek-snapshot-{timestamp}.txt"));
        let content = format!(
            "Dofek snapshot — {timestamp}\n\
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

/// Terminate a process by PID. Uses TerminateProcess on Windows, SIGTERM on Unix.
fn kill_process_by_pid(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
        use windows::Win32::Foundation::CloseHandle;
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
                .map_err(|e| format!("Access denied or process not found: {e}"))?;
            let result = TerminateProcess(handle, 1);
            let _ = CloseHandle(handle);
            result.map_err(|e| format!("TerminateProcess failed: {e}"))
        }
    }
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
            .map_err(|e| format!("kill({pid}, SIGTERM) failed: {e}"))
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
