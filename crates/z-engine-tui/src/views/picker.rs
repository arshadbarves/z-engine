//! Startup session picker (`--resume`): minimal standalone TUI that lists
//! persisted sessions newest-first and returns the chosen file.

use std::io::{self, Stdout};

use crossterm::ExecutableCommand;
use crossterm::event::{self, Event as CtEvent, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use z_engine_core::session::{self, SessionSummary};

pub fn pick_interactive() -> io::Result<Option<std::path::PathBuf>> {
    let mut sessions: Vec<SessionSummary> = Vec::new();
    for dir in z_engine_core::config::session_search_dirs() {
        sessions.extend(session::list_sessions(&dir));
    }
    sessions.sort_by_key(|s| std::cmp::Reverse(s.modified));
    if sessions.is_empty() {
        return Ok(None);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    // Panic/early-return safe: the guard restores the terminal however we
    // leave this scope.
    struct TermGuard;
    impl Drop for TermGuard {
        fn drop(&mut self) {
            let _ = std::io::stdout().execute(LeaveAlternateScreen);
            let _ = disable_raw_mode();
        }
    }
    let _guard = TermGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;
    run_loop(&mut term, &sessions)
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
