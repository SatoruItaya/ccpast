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
        _ => {}
    }
}

fn render(f: &mut ratatui::Frame, app: &App) {
    use ratatui::style::{Modifier, Style};
    use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|m| {
            let mark = if m.cwd_exists { "✓" } else { "✗" };
            ListItem::new(format!("{mark}  {}", m.title))
        })
        .collect();

    let mut state = ListState::default();
    if !app.sessions.is_empty() {
        state.select(Some(app.selected.min(app.sessions.len() - 1)));
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("ccpast"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(list, f.area(), &mut state);
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
