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
}

pub fn render(f: &mut Frame, area: Rect, view: ListView<'_>) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let list_area = layout[0];
    let status_area = layout[1];
    let help_area = layout[2];

    render_list(f, list_area, &view);
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
