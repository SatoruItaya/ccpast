use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::session::SessionMeta;
use crate::util::{format_local_short, truncate_to_width};

pub struct ListView<'a> {
    pub sessions: &'a [SessionMeta],
    pub selected: usize,
    pub show_preview: bool,
}

pub fn render(f: &mut Frame, area: Rect, view: ListView<'_>) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    let body = outer[0];
    let status_area = outer[1];
    let help_area = outer[2];

    if view.show_preview {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(body);
        render_list(f, cols[0], &view);
        render_preview(f, cols[1], &view);
    } else {
        render_list(f, body, &view);
    }

    render_status(f, status_area, &view);
    render_help(f, help_area);
}

fn render_list(f: &mut Frame, area: Rect, view: &ListView<'_>) {
    let width = area.width as usize;
    let items: Vec<ListItem> = view
        .sessions
        .iter()
        .map(|m| ListItem::new(format_row(m, width)))
        .collect();

    let mut state = ListState::default();
    if !view.sessions.is_empty() {
        state.select(Some(view.selected.min(view.sessions.len() - 1)));
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Sessions "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, area, &mut state);
}

fn format_row(m: &SessionMeta, width: usize) -> String {
    let mark = if m.cwd_exists { "✓" } else { "✗" };
    let date = format_local_short(m.last_activity);
    let base = m
        .cwd
        .as_deref()
        .and_then(|c| std::path::Path::new(c).file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "?".into());

    // Fixed widths: mark(1) + sp(2) + date(16) + sp(2) + base(<=24) + sp(2)
    // Remaining is for the title (>= 10).
    let base_trim = truncate_to_width(&base, 24);
    let used = 1 + 2 + 16 + 2 + 24 + 2 + 2; // border padding budget
    let title_width = width.saturating_sub(used).max(10);
    let title = truncate_to_width(&m.title, title_width);
    format!("{mark}  {date}  {base_trim:<24}  {title}")
}

fn render_status(f: &mut Frame, area: Rect, view: &ListView<'_>) {
    let line = if let Some(m) = view.sessions.get(view.selected) {
        let cwd = m.cwd.as_deref().unwrap_or("(no cwd)");
        let count = format!("{}/{}", view.selected + 1, view.sessions.len());
        format!("{cwd}    {count}    {} msgs", m.message_count)
    } else {
        String::from("(no sessions found)")
    };
    let p = Paragraph::new(Line::raw(line));
    f.render_widget(p, area);
}

fn render_help(f: &mut Frame, area: Rect) {
    let help =
        "↑/↓ move   Enter view   r resume   f fork   d delete   / filter   p preview   q quit";
    let p = Paragraph::new(Line::raw(help));
    f.render_widget(p, area);
}

fn render_preview(f: &mut Frame, area: Rect, view: &ListView<'_>) {
    use ratatui::text::Text;

    let body = match view.sessions.get(view.selected) {
        Some(m) => match crate::reader::load_turns(&m.path, Some(6)) {
            Ok(turns) => format_preview(&turns),
            Err(_) => "(failed to read session)".into(),
        },
        None => "(no selection)".into(),
    };
    let p = Paragraph::new(Text::raw(body))
        .block(Block::default().borders(Borders::ALL).title(" Preview "))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(p, area);
}

fn format_preview(turns: &[crate::reader::Turn]) -> String {
    let mut s = String::new();
    for t in turns {
        let header = match t.role {
            crate::reader::Role::User => "▍ user",
            crate::reader::Role::Assistant => "▍ assistant",
        };
        s.push_str(header);
        s.push('\n');
        s.push_str(&t.body);
        s.push_str("\n\n");
    }
    s
}
