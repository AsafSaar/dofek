use ratatui::style::{Color, Modifier, Style};

// Brand palette from spec
pub const ACCENT_INDIGO: Color = Color::Rgb(0x6C, 0x8E, 0xF5);
pub const ACCENT_TEAL: Color = Color::Rgb(0x4E, 0xC9, 0xA0);
pub const ACCENT_CORAL: Color = Color::Rgb(0xE5, 0x7C, 0x6A);
pub const ACCENT_PURPLE: Color = Color::Rgb(0xC9, 0x7B, 0xDC);
pub const ACCENT_AMBER: Color = Color::Rgb(0xF0, 0xB3, 0x5A);
pub const ACCENT_BLUE: Color = Color::Rgb(0x4A, 0xA8, 0xE0);
pub const AI_BADGE: Color = Color::Rgb(0x9B, 0x7F, 0xDC);

// Background
pub const BG_PRIMARY: Color = Color::Rgb(0x0E, 0x11, 0x17);
pub const BG_PANEL: Color = Color::Rgb(0x13, 0x16, 0x1F);
pub const BORDER: Color = Color::Rgb(0x22, 0x26, 0x3A);

// Text
pub const TEXT_PRIMARY: Color = Color::Rgb(0xC8, 0xCD, 0xD6);
pub const TEXT_SECONDARY: Color = Color::Rgb(0x88, 0x92, 0xA4);
pub const TEXT_DIM: Color = Color::Rgb(0x4A, 0x50, 0x68);

// Semantic aliases
pub const CPU_COLOR: Color = ACCENT_INDIGO;
pub const MEM_COLOR: Color = ACCENT_TEAL;
pub const GPU_COLOR: Color = ACCENT_CORAL;
pub const VRAM_COLOR: Color = ACCENT_PURPLE;
pub const NET_COLOR: Color = ACCENT_AMBER;
pub const DISK_COLOR: Color = ACCENT_BLUE;

// Commonly used styles
pub fn panel_block_style() -> Style {
    Style::default().fg(BORDER).bg(BG_PANEL)
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
    Style::default().fg(ACCENT_INDIGO).add_modifier(Modifier::BOLD)
}
