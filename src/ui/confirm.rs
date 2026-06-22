use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn render(f: &mut Frame, area: Rect, title: &str, message: &str) {
    let modal_area = centered_rect(60, 7, area);
    f.render_widget(Clear, modal_area);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(modal_area);

    let body = Paragraph::new(Line::raw(message.to_string())).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title} ")),
    );
    f.render_widget(body, layout[0]);

    let prompt = Paragraph::new(Line::from(vec![
        Span::styled("y", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" delete    "),
        Span::styled("n / Esc", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" cancel"),
    ]));
    f.render_widget(prompt, layout[2]);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
