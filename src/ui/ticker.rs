use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::data::process::AiState;
use crate::ui::theme;

/// Render the top ticker bar (2 lines): metric pills + AI badge + hostname/clock.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    if area.height < 1 {
        return;
    }

    // Line 1: logo + pills
    let mut spans: Vec<Span> = Vec::new();

    // Logo
    spans.push(Span::styled(" dofek", Style::default().fg(theme::CPU_COLOR).add_modifier(Modifier::BOLD)));
    spans.push(Span::styled(" v0.5", Style::default().fg(theme::TEXT_DIM)));
    spans.push(Span::styled(" │ ", Style::default().fg(theme::BORDER2)));

    // CPU pill
    let cpu_val = app.data.cpu.total_load;
    pill(&mut spans, "CPU", &format!("{:.1}%", cpu_val), val_color(cpu_val, theme::CPU_COLOR));

    // GPU pill
    if let Some(gpu) = app.primary_gpu() {
        pill(&mut spans, "GPU", &format!("{:.1}%", gpu.utilization), val_color(gpu.utilization, theme::GPU_COLOR));

        // VRAM pill
        pill(&mut spans, "VRAM", &format!("{:.0}/{:.0}MB", gpu.vram_used_mb, gpu.vram_total_mb), theme::GPU_COLOR);
    }

    // MEM pill
    let mem_pct = app.data.memory.used_percent;
    pill(&mut spans, "MEM", &format!("{:.1}%", mem_pct), val_color(mem_pct, theme::MEM_COLOR));

    // TEMP pill
    if let Some(gpu) = app.primary_gpu() {
        if gpu.temperature > 0.0 {
            pill(&mut spans, "TEMP", &format!("{:.0}°C", gpu.temperature), val_color(gpu.temperature, theme::WARN_COLOR));
        }
    }

    // NET pill
    if let Some(iface) = app.data.network.interfaces.first() {
        spans.push(Span::styled("NET ", Style::default().fg(theme::TEXT_DIM)));
        spans.push(Span::styled(
            format!("↓{}", format_rate(iface.rx_bytes_per_sec)),
            Style::default().fg(theme::NET_RX_COLOR),
        ));
        spans.push(Span::styled(" ", Style::default()));
        spans.push(Span::styled(
            format!("↑{}", format_rate(iface.tx_bytes_per_sec)),
            Style::default().fg(theme::NET_TX_COLOR),
        ));
        spans.push(Span::styled(" │ ", Style::default().fg(theme::BORDER2)));
    }

    // Plugin metrics
    for status in &app.data.plugin_statuses {
        if let Some(ref response) = status.response {
            for metric in &response.metrics {
                pill(&mut spans, &metric.label, &format!("{:.1}{}", metric.value, metric.unit), theme::TEXT_SECONDARY);
            }
        }
    }

    // AI badge
    let ai_procs: Vec<_> = app.data.processes.iter()
        .filter(|p| p.is_ai_workload && p.ai_state != AiState::None)
        .collect();

    if let Some(proc) = ai_procs.first() {
        let (dot, color) = match proc.ai_state {
            AiState::Inferring => ("●", theme::AI_COLOR),
            AiState::Loading => ("●", theme::WARN_COLOR),
            AiState::Idle => ("○", theme::TEXT_DIM),
            AiState::None => ("", theme::TEXT_DIM),
        };
        spans.push(Span::styled(dot, Style::default().fg(color)));
        spans.push(Span::styled(
            format!(" {} — {}", proc.name, proc.ai_state),
            Style::default().fg(color),
        ));
    }

    let main_line = Line::from(spans);
    let paragraph = Paragraph::new(main_line).style(Style::default().bg(theme::BG_SURFACE));
    f.render_widget(paragraph, Rect::new(area.x, area.y, area.width, 1));

    // Right-aligned hostname + clock (overlay on same line)
    let hostname = app.data.hostname.as_str();
    let clock = format_clock();
    let right_text = format!("{hostname}  {clock} ");
    let right_len = right_text.len() as u16;
    if area.width > right_len + 10 {
        let right_area = Rect::new(area.x + area.width - right_len, area.y, right_len, 1);
        let right = Paragraph::new(Line::from(vec![
            Span::styled(hostname, Style::default().fg(theme::TEXT_DIM)),
            Span::styled("  ", Style::default()),
            Span::styled(&clock, Style::default().fg(theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
        ]));
        f.render_widget(right, right_area);
    }

    // Line 2: subtle border
    if area.height >= 2 {
        let border_area = Rect::new(area.x, area.y + 1, area.width, 1);
        let border = Paragraph::new("─".repeat(area.width as usize))
            .style(Style::default().fg(theme::BORDER));
        f.render_widget(border, border_area);
    }
}

/// Render a metric pill: "LABEL value │ "
fn pill(spans: &mut Vec<Span<'static>>, label: &str, value: &str, color: ratatui::style::Color) {
    spans.push(Span::styled(format!("{label} "), Style::default().fg(theme::TEXT_DIM)));
    spans.push(Span::styled(value.to_string(), Style::default().fg(color).add_modifier(Modifier::BOLD)));
    spans.push(Span::styled(" │ ", Style::default().fg(theme::BORDER2)));
}

fn val_color(value: f32, normal: ratatui::style::Color) -> ratatui::style::Color {
    if value >= 90.0 { theme::CRIT_COLOR } else if value >= 80.0 { theme::WARN_COLOR } else { normal }
}

fn format_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_000_000.0 {
        format!("{:.1}MB/s", bytes_per_sec / 1_000_000.0)
    } else if bytes_per_sec >= 1_000.0 {
        format!("{:.1}KB/s", bytes_per_sec / 1_000.0)
    } else {
        format!("{:.0}B/s", bytes_per_sec)
    }
}

/// Cached clock string — only reformats when the second changes.
fn format_clock() -> String {
    use std::cell::RefCell;
    use std::time::Instant;

    thread_local! {
        static CACHE: RefCell<(String, Instant)> = RefCell::new((String::new(), Instant::now()));
    }

    CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        let elapsed = cache.1.elapsed();
        if elapsed.as_millis() >= 900 || cache.0.is_empty() {
            cache.1 = Instant::now();
            #[cfg(windows)]
            {
                use windows::Win32::System::SystemInformation::GetLocalTime;
                let t = unsafe { GetLocalTime() };
                cache.0 = format!("{:02}:{:02}:{:02}", t.wHour, t.wMinute, t.wSecond);
            }
            #[cfg(not(windows))]
            {
                use std::time::SystemTime;
                let secs = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let h = (secs % 86400) / 3600;
                let m = (secs % 3600) / 60;
                let s = secs % 60;
                cache.0 = format!("{h:02}:{m:02}:{s:02}");
            }
        }
        cache.0.clone()
    })
}
