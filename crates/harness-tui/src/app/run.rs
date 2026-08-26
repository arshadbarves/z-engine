use std::path::Path;

use crossterm::event::{Event as CtEvent, EventStream, KeyEventKind};
use futures::StreamExt;
use harness_core::agent::{AgentHandle, EventRx, PermissionMode};
use harness_core::config::Config;
use ratatui::Terminal;

use super::{App, Block};
use crate::views;

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

pub async fn run(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    handle: AgentHandle,
    events: EventRx,
    config: Config,
    project_root: &Path,
    session_tag: String,
    initial_mode: PermissionMode,
) -> anyhow::Result<()> {
    use crossterm::ExecutableCommand;
    use crossterm::event::{DisableMouseCapture, EnableMouseCapture};

    std::io::stdout().execute(EnableMouseCapture)?;
    let mut app = App::new(
        handle,
        events,
        &config,
        project_root,
        session_tag,
        initial_mode,
    );
    let mut reader = EventStream::new();

    app.blocks.push(Block::Notice(format!(
        "harness v{} · model {}\nproject {}\ntype a task + Enter · Esc aborts · PgUp/PgDn scrolls · Ctrl-C twice quits",
        env!("CARGO_PKG_VERSION"),
        config.model,
        project_root.display()
    )));

    let mut tick = tokio::time::interval(std::time::Duration::from_millis(500));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let result = loop {
        while let Some(ev) = app.events.try_recv() {
            app.on_core_event(ev);
        }

        terminal.draw(|f| views::render(f, &app))?;
        if app.should_quit {
            break Ok(());
        }

        let height = terminal.size().map(|s| s.height)?.saturating_sub(4).max(1);

        tokio::select! {
            maybe = reader.next() => match maybe {
                Some(Ok(CtEvent::Key(k))) => {
                    if k.kind != KeyEventKind::Release {
                        app.on_key(height, k);
                    }
                }
                Some(Ok(other)) => app.on_ct_event(height, other),
                Some(Err(e)) => break Err(e.into()),
                None => break Ok(()),
            },
            ev = app.events.recv() => match ev {
                Some(e) => app.on_core_event(e),
                None => break Ok(()),
            },
            _ = tick.tick() => {} // drives the working-Ns spinner
        }
    };

    std::io::stdout().execute(DisableMouseCapture)?;
    result
}
