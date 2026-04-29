use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(80),
            Constraint::Percentage(20),
        ])
        .split(area);

    let keybindings = Line::from(vec![
        Span::styled("q", Style::default().fg(theme::ACCENT_INDIGO)),
        Span::styled(" quit  ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled("tab", Style::default().fg(theme::ACCENT_INDIGO)),
        Span::styled(" sort  ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled("p", Style::default().fg(theme::ACCENT_INDIGO)),
        Span::styled(" proc  ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled("g", Style::default().fg(theme::ACCENT_INDIGO)),
        Span::styled(" gpu  ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled("c", Style::default().fg(theme::ACCENT_INDIGO)),
        Span::styled(" cpu  ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled("m", Style::default().fg(theme::ACCENT_INDIGO)),
        Span::styled(" mem  ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled("?", Style::default().fg(theme::ACCENT_INDIGO)),
        Span::styled(" help", Style::default().fg(theme::TEXT_DIM)),
    ]);
    f.render_widget(Paragraph::new(keybindings), chunks[0]);

    let refresh = Line::from(Span::styled(
        format!("{}ms", app.refresh_ms.load(std::sync::atomic::Ordering::Relaxed)),
        Style::default().fg(theme::TEXT_DIM),
    ));
    f.render_widget(Paragraph::new(refresh).alignment(Alignment::Right), chunks[1]);
}
