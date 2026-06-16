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
    should_quit: bool,
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
            should_quit: false,
        }
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
        Mode::List => match code {
            KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => {
                if !app.sessions.is_empty() && app.selected + 1 < app.sessions.len() {
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
                if let Some(m) = app.sessions.get(app.selected) {
                    if let Ok(turns) = crate::reader::load_turns(&m.path, None) {
                        app.mode = Mode::Reader { turns, scroll: 0 };
                    }
                }
            }
            _ => {}
        },
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
            list::render(
                f,
                f.area(),
                list::ListView {
                    sessions: &app.sessions,
                    selected: app.selected,
                    show_preview: show,
                },
            );
        }
        Mode::Reader { turns, scroll } => {
            if let Some(m) = app.sessions.get(app.selected) {
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
