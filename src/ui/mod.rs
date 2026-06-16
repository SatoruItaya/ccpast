mod list;
mod reader;

use std::io::{self, Stdout};
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
                    handle_key(key.code, &mut app);
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
    should_quit: bool,
}

struct FilterState {
    active: bool,
    query: String,
}

enum Mode {
    List,
    Reader { turns: Vec<Turn>, scroll: u16 },
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
            },
            reader_index: None,
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
                m.title.to_lowercase().contains(&q)
                    || m.cwd
                        .as_deref()
                        .map(|c| c.to_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

fn effective_preview(app: &App, width: u16) -> bool {
    match app.show_preview {
        Some(v) => v,
        None => width >= 100,
    }
}

fn handle_key(code: KeyCode, app: &mut App) {
    match &mut app.mode {
        Mode::List => {
            if app.filter.active {
                match code {
                    KeyCode::Esc => {
                        app.filter.active = false;
                        app.filter.query.clear();
                    }
                    KeyCode::Enter => {
                        app.filter.active = false;
                    }
                    KeyCode::Backspace => {
                        app.filter.query.pop();
                    }
                    KeyCode::Char(c) => app.filter.query.push(c),
                    _ => {}
                }
                return;
            }
            match code {
                KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                KeyCode::Char('/') => {
                    app.filter.active = true;
                    app.filter.query.clear();
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
                _ => {}
            }
        }
        Mode::Reader { scroll, .. } => match code {
            KeyCode::Char('q') | KeyCode::Esc => app.mode = Mode::List,
            KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
            KeyCode::PageDown => *scroll = scroll.saturating_add(10),
            KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
            _ => {}
        },
    }
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
                },
            );
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
