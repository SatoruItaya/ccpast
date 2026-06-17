mod confirm;
mod list;
mod reader;

use std::collections::HashMap;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::reader::Turn;
use crate::session::SessionMeta;

pub fn run(sessions: Vec<SessionMeta>) -> Result<()> {
    let mut _guard = RawModeGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(sessions);
    while !app.should_quit {
        terminal.draw(|f| render(f, &mut app))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match handle_key(key.code, &mut app) {
                        Action::None => {}
                        Action::Resume { fork } => do_resume(&mut app, &mut terminal, fork)?,
                        Action::LoadBodies => {
                            let total = app.sessions.len();
                            app.status = Some(format!("reading bodies ({total} sessions)…"));
                            terminal.draw(|f| render(f, &mut app))?;

                            let mut cache: std::collections::HashMap<std::path::PathBuf, String> =
                                std::collections::HashMap::with_capacity(total);
                            for m in &app.sessions {
                                let body = match crate::reader::load_turns(&m.path, None) {
                                    Ok(turns) => turns
                                        .iter()
                                        .map(|t| t.body.as_str())
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                        .to_lowercase(),
                                    Err(_) => String::new(),
                                };
                                cache.insert(m.path.clone(), body);
                            }
                            app.filter.body_cache = Some(cache);
                            app.filter.body_scope = true;
                            app.status = Some(format!("body search enabled ({total} sessions)"));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

struct App {
    sessions: Vec<SessionMeta>,
    selected: usize,
    show_preview: Option<bool>,
    last_width: u16,
    mode: Mode,
    filter: FilterState,
    reader_index: Option<usize>,
    status: Option<String>,
    pending_delete: Option<usize>,
    should_quit: bool,
}

struct FilterState {
    active: bool,
    query: String,
    body_scope: bool,
    body_cache: Option<HashMap<PathBuf, String>>,
}

enum Mode {
    List,
    Reader { turns: Vec<Turn>, scroll: u16 },
}

enum Action {
    None,
    Resume { fork: bool },
    LoadBodies,
}

impl App {
    fn new(sessions: Vec<SessionMeta>) -> Self {
        Self {
            sessions,
            selected: 0,
            show_preview: None,
            last_width: 0,
            mode: Mode::List,
            filter: FilterState {
                active: false,
                query: String::new(),
                body_scope: false,
                body_cache: None,
            },
            reader_index: None,
            status: None,
            pending_delete: None,
            should_quit: false,
        }
    }

    fn filtered_indices(&self) -> Vec<usize> {
        if self.filter.query.is_empty() {
            return (0..self.sessions.len()).collect();
        }
        let q = self.filter.query.to_lowercase();
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                match_session(
                    m,
                    &q,
                    self.filter.body_scope,
                    self.filter.body_cache.as_ref(),
                )
            })
            .map(|(i, _)| i)
            .collect()
    }
}

/// Decide whether a single session matches the active filter.
/// `query_lower` is assumed to be already lowercased.
/// `body_cache` is the optional cache from `FilterState`; bodies inside are
/// expected to be already lowercased at load time.
fn match_session(
    meta: &SessionMeta,
    query_lower: &str,
    body_scope: bool,
    body_cache: Option<&std::collections::HashMap<std::path::PathBuf, String>>,
) -> bool {
    let title_hit = meta.title.to_lowercase().contains(query_lower);
    let cwd_hit = meta
        .cwd
        .as_deref()
        .map(|c| c.to_lowercase().contains(query_lower))
        .unwrap_or(false);
    let body_hit = body_scope
        && body_cache
            .and_then(|c| c.get(&meta.path))
            .map(|b| b.contains(query_lower))
            .unwrap_or(false);
    title_hit || cwd_hit || body_hit
}

fn effective_preview(app: &App, width: u16) -> bool {
    match app.show_preview {
        Some(v) => v,
        None => width >= 100,
    }
}

fn handle_key(code: KeyCode, app: &mut App) -> Action {
    app.status = None;
    match &mut app.mode {
        Mode::List => {
            if app.pending_delete.is_some() {
                match code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        let real_idx = match app.pending_delete.take() {
                            Some(i) => i,
                            None => return Action::None,
                        };
                        if real_idx >= app.sessions.len() {
                            app.status = Some("invalid selection".into());
                            return Action::None;
                        }
                        let meta = app.sessions[real_idx].clone();
                        let trash_root = match crate::scan::projects_root()
                            .and_then(|r| r.parent().map(|p| p.join(".trash")))
                        {
                            Some(p) => p,
                            None => {
                                app.status = Some("cannot determine ~/.claude/.trash".into());
                                return Action::None;
                            }
                        };
                        match crate::trash::move_to_trash(&trash_root, &meta.path, &meta.session_id)
                        {
                            Ok(_) => {
                                app.sessions.remove(real_idx);
                                let new_count = app.filtered_indices().len();
                                if app.selected >= new_count {
                                    app.selected = new_count.saturating_sub(1);
                                }
                                app.status = Some(format!("moved {} to trash", meta.session_id));
                            }
                            Err(err) => app.status = Some(format!("delete failed: {err:#}")),
                        }
                        return Action::None;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        app.pending_delete = None;
                        return Action::None;
                    }
                    _ => return Action::None,
                }
            }
            if app.filter.active {
                match code {
                    KeyCode::Esc => {
                        app.filter.active = false;
                        app.filter.query.clear();
                        app.filter.body_scope = false;
                    }
                    KeyCode::Enter => {
                        app.filter.active = false;
                    }
                    KeyCode::Backspace => {
                        app.filter.query.pop();
                    }
                    KeyCode::Char(c) => app.filter.query.push(c),
                    KeyCode::Tab => {
                        if app.filter.body_cache.is_none() {
                            return Action::LoadBodies;
                        }
                        app.filter.body_scope = !app.filter.body_scope;
                        return Action::None;
                    }
                    _ => {}
                }
                return Action::None;
            }
            match code {
                KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                KeyCode::Char('/') => {
                    app.filter.active = true;
                    app.filter.query.clear();
                    app.filter.body_scope = false;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let indices = app.filtered_indices();
                    if !indices.is_empty() && app.selected + 1 < indices.len() {
                        app.selected += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.selected = app.selected.saturating_sub(1);
                }
                KeyCode::Char('p') => {
                    let cur = effective_preview(app, app.last_width);
                    app.show_preview = Some(!cur);
                }
                KeyCode::Enter => {
                    let indices = app.filtered_indices();
                    if let Some(&real_idx) = indices.get(app.selected) {
                        let m = &app.sessions[real_idx];
                        if let Ok(turns) = crate::reader::load_turns(&m.path, None) {
                            app.reader_index = Some(real_idx);
                            app.mode = Mode::Reader { turns, scroll: 0 };
                        }
                    }
                }
                KeyCode::Char('r') => return Action::Resume { fork: false },
                KeyCode::Char('f') => return Action::Resume { fork: true },
                KeyCode::Char('d') => {
                    let indices = app.filtered_indices();
                    if let Some(&real_idx) = indices.get(app.selected) {
                        app.pending_delete = Some(real_idx);
                    }
                }
                _ => {}
            }
        }
        Mode::Reader { scroll, .. } => match code {
            KeyCode::Char('q') | KeyCode::Esc => app.mode = Mode::List,
            KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
            KeyCode::PageDown => *scroll = scroll.saturating_add(10),
            KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
            KeyCode::Char('r') => return Action::Resume { fork: false },
            KeyCode::Char('f') => return Action::Resume { fork: true },
            _ => {}
        },
    }
    Action::None
}

fn render(f: &mut ratatui::Frame, app: &mut App) {
    app.last_width = f.area().width;
    match &app.mode {
        Mode::List => {
            let show = effective_preview(app, app.last_width);
            let indices = app.filtered_indices();
            if app.selected >= indices.len() {
                app.selected = indices.len().saturating_sub(1);
            }
            list::render(
                f,
                f.area(),
                list::ListView {
                    sessions: &app.sessions,
                    indices: &indices,
                    cursor: app.selected,
                    show_preview: show,
                    filter_input: app.filter.active.then_some(app.filter.query.as_str()),
                    status_override: app.status.as_deref(),
                    body_scope: app.filter.body_scope,
                },
            );
            if let Some(real_idx) = app.pending_delete {
                if let Some(m) = app.sessions.get(real_idx) {
                    let msg = format!(
                        "Move session \"{}\" ({}) to ~/.claude/.trash/ ?",
                        crate::util::truncate_to_width(&m.title, 50),
                        m.session_id
                    );
                    confirm::render(f, f.area(), "Confirm delete", &msg);
                }
            }
        }
        Mode::Reader { turns, scroll } => {
            if let Some(idx) = app.reader_index {
                if let Some(m) = app.sessions.get(idx) {
                    reader::render(
                        f,
                        f.area(),
                        reader::ReaderView {
                            meta: m,
                            turns,
                            scroll: *scroll,
                        },
                    );
                }
            }
        }
    }
}

fn do_resume(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    fork: bool,
) -> Result<()> {
    let real_idx = match &app.mode {
        Mode::List => {
            let indices = app.filtered_indices();
            indices.get(app.selected).copied()
        }
        Mode::Reader { .. } => app.reader_index,
    };
    let Some(real_idx) = real_idx else {
        return Ok(());
    };
    let m = app.sessions[real_idx].clone();
    let Some(cwd) = m.cwd.clone() else {
        app.status = Some("session has no recorded cwd".into());
        return Ok(());
    };

    // Tear down before spawning.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    let result = crate::resume::spawn(std::path::Path::new(&cwd), &m.session_id, fork);

    if result.is_ok() {
        std::process::exit(0);
    }

    // Re-enter the TUI to display the error.
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;

    match result {
        Ok(()) => {}
        Err(e) => app.status = Some(format!("resume failed: {e}")),
    }
    Ok(())
}

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let mut stdout: Stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn meta(title: &str, cwd: Option<&str>, path: &str) -> SessionMeta {
        SessionMeta {
            session_id: "id".into(),
            path: PathBuf::from(path),
            cwd: cwd.map(String::from),
            cwd_exists: true,
            last_activity: chrono::Utc::now(),
            title: title.into(),
            message_count: 0,
        }
    }

    #[test]
    fn title_substring_matches_case_insensitive() {
        let m = meta("Implement TASK 4", Some("/p"), "/p/x.jsonl");
        assert!(match_session(&m, "task", false, None));
    }

    #[test]
    fn cwd_substring_matches() {
        let m = meta("(no title)", Some("/home/user/proj"), "/x.jsonl");
        assert!(match_session(&m, "proj", false, None));
    }

    #[test]
    fn body_scope_off_ignores_body_cache() {
        let m = meta("(no title)", Some("/p"), "/x.jsonl");
        let mut cache = HashMap::new();
        cache.insert(PathBuf::from("/x.jsonl"), "hello world".into());
        assert!(!match_session(&m, "hello", false, Some(&cache)));
    }

    #[test]
    fn body_scope_on_with_body_hit_matches() {
        let m = meta("(no title)", Some("/p"), "/x.jsonl");
        let mut cache = HashMap::new();
        cache.insert(PathBuf::from("/x.jsonl"), "hello world".into());
        assert!(match_session(&m, "hello", true, Some(&cache)));
    }

    #[test]
    fn body_scope_on_with_missing_cache_entry_does_not_match() {
        let m = meta("(no title)", Some("/p"), "/x.jsonl");
        let cache: HashMap<PathBuf, String> = HashMap::new();
        assert!(!match_session(&m, "hello", true, Some(&cache)));
    }

    #[test]
    fn body_scope_on_still_matches_title() {
        let m = meta("Hello World", Some("/p"), "/x.jsonl");
        let cache: HashMap<PathBuf, String> = HashMap::new();
        assert!(match_session(&m, "hello", true, Some(&cache)));
    }
}
