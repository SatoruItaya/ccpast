use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::reader::{Role, Turn};
use crate::session::SessionMeta;

pub struct ReaderView<'a> {
    pub meta: &'a SessionMeta,
    pub turns: &'a [Turn],
    pub scroll: u16,
}

pub fn render(f: &mut Frame, area: Rect, view: ReaderView<'_>) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let body_area = layout[0];
    let help_area = layout[1];

    let text = build_text(view.turns);
    let title = format!(
        " {} — {} ",
        view.meta.session_id,
        view.meta.cwd.as_deref().unwrap_or("(no cwd)")
    );
    let p = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((view.scroll, 0));
    f.render_widget(p, body_area);

    let help = Paragraph::new(Line::raw(
        "↑/↓ scroll   PgUp/PgDn page   r resume   q/Esc back",
    ));
    f.render_widget(help, help_area);
}

fn build_text(turns: &[Turn]) -> Text<'_> {
    let mut lines: Vec<Line<'_>> = Vec::new();
    for t in turns {
        let header = match t.role {
            Role::User => "▍ user",
            Role::Assistant => "▍ assistant",
        };
        lines.push(Line::from(Span::styled(
            header.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for body_line in t.body.lines() {
            lines.push(Line::raw(body_line.to_string()));
        }
        lines.push(Line::raw(""));
    }
    Text::from(lines)
}
