mod list;

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

use crate::session::SessionMeta;

pub fn run(sessions: Vec<SessionMeta>) -> Result<()> {
    let mut _guard = RawModeGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App {
        sessions,
        selected: 0,
        should_quit: false,
    };
    while !app.should_quit {
        terminal.draw(|f| render(f, &app))?;
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
    should_quit: bool,
}

fn handle_key(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.sessions.is_empty() && app.selected + 1 < app.sessions.len() {
                app.selected += 1;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected > 0 {
                app.selected = app.selected.saturating_sub(1);
            }
        }
        _ => {}
    }
}

fn render(f: &mut ratatui::Frame, app: &App) {
    list::render(
        f,
        f.area(),
        list::ListView {
            sessions: &app.sessions,
            selected: app.selected,
        },
    );
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
