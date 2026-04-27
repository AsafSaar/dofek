use ratatui::style::{Color, Modifier, Style};

// Trading-terminal inspired palette (v2)
pub const CPU_COLOR: Color = Color::Rgb(0x38, 0xBD, 0xF8);     // Sky blue
pub const GPU_COLOR: Color = Color::Rgb(0xA7, 0x8B, 0xFA);     // Violet
pub const MEM_COLOR: Color = Color::Rgb(0x34, 0xD3, 0x99);     // Emerald
pub const NET_RX_COLOR: Color = Color::Rgb(0x38, 0xBD, 0xF8);  // Sky blue (same as CPU)
pub const NET_TX_COLOR: Color = Color::Rgb(0xFB, 0x92, 0x3C);  // Orange
pub const AI_COLOR: Color = Color::Rgb(0xC0, 0x84, 0xFC);      // Purple
pub const DEV_COLOR: Color = Color::Rgb(0x60, 0xA5, 0xFA);     // Blue
pub const WATCH_COLOR: Color = Color::Rgb(0xFB, 0xBF, 0x24);   // Amber
pub const WARN_COLOR: Color = Color::Rgb(0xFB, 0xBF, 0x24);    // Amber (>80%)
pub const CRIT_COLOR: Color = Color::Rgb(0xF8, 0x71, 0x71);    // Red (>90%)
pub const GREEN_COLOR: Color = Color::Rgb(0x4A, 0xDE, 0x80);   // Green (positive delta)

// Background
pub const BG_PRIMARY: Color = Color::Rgb(0x06, 0x08, 0x10);
pub const BG_SURFACE: Color = Color::Rgb(0x0B, 0x11, 0x20);
pub const BG_SURFACE2: Color = Color::Rgb(0x0F, 0x19, 0x29);
pub const BORDER: Color = Color::Rgb(0x18, 0x20, 0x35);
pub const BORDER2: Color = Color::Rgb(0x1F, 0x2D, 0x48);

// Text
pub const TEXT_PRIMARY: Color = Color::Rgb(0xE2, 0xE8, 0xF0);
pub const TEXT_SECONDARY: Color = Color::Rgb(0x94, 0xA3, 0xB8);
pub const TEXT_DIM: Color = Color::Rgb(0x3D, 0x50, 0x70);

// Backward-compat aliases (used by existing panel code until fully migrated)
pub const ACCENT_INDIGO: Color = CPU_COLOR;
pub const ACCENT_TEAL: Color = MEM_COLOR;
pub const ACCENT_CORAL: Color = GPU_COLOR;
pub const ACCENT_BLUE: Color = DEV_COLOR;
pub const VRAM_COLOR: Color = GPU_COLOR;
pub const NET_COLOR: Color = NET_TX_COLOR;
pub const DISK_COLOR: Color = Color::Rgb(0xEA, 0xB3, 0x08);   // Amber (distinct from NET orange + CPU blue)
pub const AI_BADGE: Color = AI_COLOR;
pub const ACCENT_PURPLE: Color = AI_COLOR;
pub const ACCENT_AMBER: Color = WARN_COLOR;
pub const BG_PANEL: Color = BG_SURFACE;

// Commonly used styles
pub fn panel_block_style() -> Style {
    Style::default().fg(BORDER).bg(BG_SURFACE)
}

pub fn title_style() -> Style {
    Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)
}

pub fn label_style() -> Style {
    Style::default().fg(TEXT_SECONDARY)
}

pub fn dim_style() -> Style {
    Style::default().fg(TEXT_DIM)
}

pub fn header_style() -> Style {
    Style::default().fg(CPU_COLOR).add_modifier(Modifier::BOLD)
}

/// Derive 3 horizon-chart color bands from a base color (dim → medium → full).
pub fn horizon_bands(base: Color) -> [Color; 3] {
    let dim = |c: Color, f: f64| -> Color {
        match c {
            Color::Rgb(r, g, b) => Color::Rgb(
                (r as f64 * f) as u8,
                (g as f64 * f) as u8,
                (b as f64 * f) as u8,
            ),
            other => other,
        }
    };
    [dim(base, 0.25), dim(base, 0.55), base]
}
