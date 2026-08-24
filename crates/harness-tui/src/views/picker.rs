//! Startup session picker (`--resume`): minimal standalone TUI that lists
//! persisted sessions newest-first and returns the chosen file.

use std::io::{self, Stdout};

use crossterm::ExecutableCommand;
use crossterm::event::{self, Event as CtEvent, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use harness_core::session::{self, SessionSummary};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn pick_interactive(sessions_dir: &std::path::Path) -> io::Result<Option<std::path::PathBuf>> {
    let sessions = session::list_sessions(sessions_dir);
    if sessions.is_empty() {
        return Ok(None);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;
    let result = run_loop(&mut term, &sessions);

    let mut stdout = io::stdout();
    stdout.execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    result
}

fn run_loop(
    term: &mut Terminal<CrosstermBackend<Stdout>>,
    sessions: &[SessionSummary],
) -> io::Result<Option<std::path::PathBuf>> {
    let mut selected = ListState::default();
    selected.select(Some(0));
    loop {
        term.draw(|f| {
            let items: Vec<ListItem> = sessions
                .iter()
                .map(|s| {
                    let preview = s
                        .first_user_msg
                        .clone()
                        .unwrap_or_else(|| "(no user msg)".into());
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{} ", &s.ulid[..s.ulid.len().min(6)]),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(preview),
                    ]))
                })
                .collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" resume session (↑↓ select · Enter open · Esc new) "),
                )
                .highlight_style(Style::default().bg(Color::DarkGray));
            f.render_stateful_widget(list, f.area(), &mut selected);
        })?;

        if let CtEvent::Key(k) = event::read()? {
            if k.kind == KeyEventKind::Release {
                continue;
            }
            match k.code {
                KeyCode::Char('q') => return Ok(None),
                KeyCode::Esc => return Ok(None),
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let next = selected
                        .selected()
                        .map(|i| (i + 1).min(sessions.len().saturating_sub(1)))
                        .unwrap_or(0);
                    selected.select(Some(next));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let prev = selected.selected().unwrap_or(0).saturating_sub(1);
                    selected.select(Some(prev));
                }
                KeyCode::Enter => {
                    return Ok(selected.selected().map(|i| sessions[i].path.clone()));
                }
                _ => {}
            }
        }
    }
}
