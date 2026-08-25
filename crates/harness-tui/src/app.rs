//! Application state + the single event loop (crossterm ⇄ core events).

use std::collections::VecDeque;
use std::path::Path;

use crossterm::event::{
    Event as CtEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use futures::StreamExt;
use harness_core::agent::{AgentHandle, ApprovalDecision, Event, EventRx};
use harness_core::config::Config;
use ratatui::Terminal;

use crate::views;

/// One rendered unit of conversation history.
#[derive(Debug, Clone)]
pub enum Block {
    User(String),
    Assistant {
        text: String,
        streaming: bool,
    },
    ToolCall {
        name: String,
        preview: String,
        summary: String,
        ok: bool,
        done: bool,
    },
    Notice(String),
    Error(String),
}

pub struct PendingApproval {
    pub id: u64,
    pub tool: String,
    pub input_preview: String,
    pub suggested_rule: Option<String>,
    /// Rich preview (unified diff) for editing tools.
    pub detail_preview: Option<String>,
    /// Whether "persist to project config" is offered.
    pub can_persist: bool,
}

pub struct App {
    pub handle: AgentHandle,
    events: EventRx,
    pub blocks: Vec<Block>,
    pub input: String,
    history: VecDeque<String>,
    history_pos: Option<usize>,
    /// Lines scrolled up from the live bottom (0 = follow).
    pub scroll_from_bottom: u16,
    pub pending: Option<PendingApproval>,
    pub turn_active: bool,
    pub model: String,
    pub max_context_tokens: u32,
    pub session_tag: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Resolved pricing for the active model, if known.
    pub pricing: Option<harness_core::context::cost::Pricing>,
    quit_hint_until: Option<std::time::Instant>,
    pub should_quit: bool,
}

impl App {
    fn new(
        handle: AgentHandle,
        events: EventRx,
        config: &Config,
        project_root: &Path,
        session_tag: String,
    ) -> Self {
        tracing::info!(project = %project_root.display(), %session_tag, "session started");
        Self {
            handle,
            events,
            blocks: Vec::new(),
            input: String::new(),
            history: VecDeque::new(),
            history_pos: None,
            scroll_from_bottom: 0,
            pending: None,
            turn_active: false,
            model: config.model.clone(),
            max_context_tokens: config.max_context_tokens,
            session_tag,
            prompt_tokens: 0,
            completion_tokens: 0,
            pricing: harness_core::context::cost::for_model(&config.model),
            quit_hint_until: None,
            should_quit: false,
        }
    }

    // ---- input handling ---------------------------------------------------

    fn on_ct_event(&mut self, area_height: u16, ev: CtEvent) {
        match ev {
            CtEvent::Key(k) => self.on_key(area_height, k),
            CtEvent::Mouse(m) => match m.kind {
                MouseEventKind::ScrollUp => self.scroll_down_by(area_height / 2),
                MouseEventKind::ScrollDown => self.scroll_up_by(area_height / 2),
                _ => {}
            },
            _ => {}
        }
    }

    fn on_key(&mut self, area_height: u16, k: KeyEvent) {
        if k.kind == KeyEventKind::Release {
            return;
        }

        // Approval modal swallows keys while visible.
        if let Some(pending) = self.pending.take() {
            let ctrl_c = k.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('C'));
            let rule_for = |p: &PendingApproval| {
                p.suggested_rule.clone().unwrap_or_else(|| {
                    harness_core::perms::PolicyEngine::suggested_rule(
                        p.input_preview
                            .get(10..)
                            .map(|s| s.trim_matches('"'))
                            .unwrap_or(""),
                    )
                })
            };
            match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.handle.approve(pending.id, ApprovalDecision::Once);
                    self.blocks.push(Block::Notice(format!(
                        "\u{2713} approved once: {} {}",
                        pending.tool, pending.input_preview
                    )));
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    let rule = rule_for(&pending);
                    self.handle.approve(
                        pending.id,
                        ApprovalDecision::AlwaysSession { rule: rule.clone() },
                    );
                    self.blocks.push(Block::Notice(format!(
                        "\u{2713} always this prefix [{rule}] (session)"
                    )));
                }
                KeyCode::Char('p') | KeyCode::Char('P') if pending.can_persist => {
                    let rule = rule_for(&pending);
                    self.handle.approve(
                        pending.id,
                        ApprovalDecision::AlwaysPersist { rule: rule.clone() },
                    );
                    self.blocks.push(Block::Notice(format!(
                        "\u{2713} persisted [{}] to .harness/config.toml",
                        rule
                    )));
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.handle.deny(pending.id);
                    self.blocks.push(Block::Notice(format!(
                        "\u{2717} denied: {} {}",
                        pending.tool, pending.input_preview
                    )));
                }
                _ if ctrl_c => {
                    // Ctrl-C during approval == deny (spec section 10).
                    self.handle.deny(pending.id);
                    self.blocks
                        .push(Block::Notice("x denied via Ctrl-C".into()));
                }
                _ => self.pending = Some(pending), // unknown key: keep modal
            }
            return;
        }

        match k.code {
            KeyCode::Enter => {
                let text = self.input.trim().to_string();
                if text.is_empty() || self.turn_active {
                    return;
                }
                // Slash commands never reach the model.
                match text.as_str() {
                    "/compact" => {
                        self.input.clear();
                        self.blocks
                            .push(Block::Notice("[compacting context…]".into()));
                        self.handle.compact();
                        return;
                    }
                    "/notes" => {
                        self.input.clear();
                        self.handle.request_notes();
                        return;
                    }
                    _ => {}
                }
                self.input.clear();
                self.history.push_back(text.clone());
                if self.history.len() > 100 {
                    self.history.pop_front();
                }
                self.history_pos = None;
                self.turn_active = true;
                self.handle.submit(text.clone());
                self.blocks.push(Block::User(text));
                self.scroll_from_bottom = 0;
            }
            KeyCode::Esc => {
                if self.turn_active {
                    self.handle.abort();
                    self.blocks.push(Block::Notice("[aborting…]".into()));
                } else {
                    self.input.clear();
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C')
                if k.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let hinting = self
                    .quit_hint_until
                    .is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(2));
                if hinting {
                    self.should_quit = true;
                    self.handle.shutdown();
                } else {
                    self.quit_hint_until = Some(std::time::Instant::now());
                    if self.turn_active {
                        self.handle.abort(); // first Ctrl-C aborts the run too
                    }
                }
            }
            KeyCode::PageUp => self.scroll_up_by(area_height / 2),
            KeyCode::PageDown => self.scroll_down_by(area_height / 2),
            KeyCode::Home => self.scroll_from_bottom = u16::MAX,
            KeyCode::End => self.scroll_from_bottom = 0,
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Up => self.history_prev(),
            KeyCode::Down => self.history_next(),
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.push(c);
            }
            _ => {}
        }
    }

    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let pos = match self.history_pos {
            None => self.history.len() - 1,
            Some(p) => p.saturating_sub(1).min(self.history.len() - 1),
        };
        self.history_pos = Some(pos);
        self.input = self.history[pos].clone();
    }

    fn history_next(&mut self) {
        match self.history_pos {
            None => {}
            Some(p) if p + 1 >= self.history.len() => {
                self.history_pos = None;
                self.input.clear();
            }
            Some(p) => {
                self.history_pos = Some(p + 1);
                self.input = self.history[p + 1].clone();
            }
        }
    }

    fn scroll_up_by(&mut self, n: u16) {
        self.scroll_from_bottom = self.scroll_from_bottom.saturating_add(n.max(1));
    }

    fn scroll_down_by(&mut self, n: u16) {
        self.scroll_from_bottom = self.scroll_from_bottom.saturating_sub(n.max(1));
    }

    // ---- core events ------------------------------------------------------

    fn on_core_event(&mut self, ev: Event) {
        match ev {
            Event::TurnStarted => {
                self.finish_streaming();
            }
            Event::TokenDelta(t) => {
                self.assistant_streaming_block().push_str(&t);
            }
            Event::ToolCallStarted { name, preview } => {
                self.finish_streaming();
                self.blocks.push(Block::ToolCall {
                    name,
                    preview,
                    summary: String::new(),
                    ok: true,
                    done: false,
                });
            }
            Event::ToolCallFinished {
                name, ok, summary, ..
            } => {
                for b in self.blocks.iter_mut().rev() {
                    if let Block::ToolCall {
                        name: n,
                        summary: s,
                        ok: o,
                        done,
                        ..
                    } = b
                    {
                        if *n == name && !*done {
                            *s = summary;
                            *o = ok;
                            *done = true;
                            break;
                        }
                    }
                }
            }
            Event::ApprovalRequired {
                id,
                tool,
                input_preview,
                suggested_rule,
                detail_preview,
                can_persist,
            } => {
                self.finish_streaming();
                self.pending = Some(PendingApproval {
                    id,
                    tool,
                    input_preview,
                    suggested_rule,
                    detail_preview,
                    can_persist,
                });
            }
            Event::UsageUpdated {
                prompt_tokens,
                completion_tokens,
            } => {
                self.prompt_tokens = prompt_tokens;
                self.completion_tokens = completion_tokens;
            }
            Event::StatusNote(s) => self.blocks.push(Block::Notice(s)),
            Event::TurnCompleted { .. } => {
                self.finish_streaming();
                self.blocks.push(Block::Notice("✓ done".into()));
                self.turn_active = false;
            }
            Event::TurnAborted => {
                self.finish_streaming();
                self.blocks.push(Block::Notice("■ aborted".into()));
                self.turn_active = false;
            }
            Event::Error(msg) => {
                self.finish_streaming();
                self.blocks.push(Block::Error(msg));
                self.turn_active = false;
            }
        }
    }

    fn assistant_streaming_block(&mut self) -> &mut String {
        if !matches!(
            self.blocks.last(),
            Some(Block::Assistant {
                streaming: true,
                ..
            })
        ) {
            self.finish_streaming();
            self.blocks.push(Block::Assistant {
                text: String::new(),
                streaming: true,
            });
        }
        match self.blocks.last_mut() {
            Some(Block::Assistant { text, .. }) => text,
            _ => unreachable!("just pushed an assistant block"),
        }
    }

    fn finish_streaming(&mut self) {
        if let Some(Block::Assistant { streaming, .. }) = self.blocks.last_mut() {
            *streaming = false;
        }
    }
}

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
) -> anyhow::Result<()> {
    use crossterm::ExecutableCommand;
    use crossterm::event::{DisableMouseCapture, EnableMouseCapture};

    std::io::stdout().execute(EnableMouseCapture)?;
    let mut app = App::new(handle, events, &config, project_root, session_tag);
    let mut reader = EventStream::new();

    app.blocks.push(Block::Notice(format!(
        "harness v{} · model {}\nproject {}\ntype a task + Enter · Esc aborts · PgUp/PgDn scrolls · Ctrl-C twice quits",
        env!("CARGO_PKG_VERSION"),
        config.model,
        project_root.display()
    )));

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
        }
    };

    std::io::stdout().execute(DisableMouseCapture)?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views;
    use harness_core::agent::{LoopConfig, spawn};
    use ratatui::{Terminal, backend::TestBackend};

    fn test_app() -> App {
        // Bogus provider URL: handles are valid, no network is touched.
        let cfg = LoopConfig::new("test-model-x", "http://127.0.0.1:1/v1");
        let (handle, ev_rx) = spawn(cfg);
        let config = Config {
            model: "test-model-x".into(),
            base_url: "http://127.0.0.1:1/v1".into(),
            max_context_tokens: 120_000,
            permissions: Default::default(),
            review_enabled: true,
            mcp_servers: vec![],
        };
        App::new(
            handle,
            ev_rx,
            &config,
            Path::new("/tmp"),
            "tst123".to_string(),
        )
    }

    fn draw(app: &App) -> String {
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| views::render(f, app)).unwrap();
        let mut out = String::new();
        for row in 0..24 {
            for col in 0..80 {
                out.push(
                    term.backend().buffer()[(col, row)]
                        .symbol()
                        .chars()
                        .next()
                        .unwrap_or(' '),
                );
            }
            out.push('\n');
        }
        out
    }

    #[tokio::test]
    async fn ui_renders_transcript_status_and_modal() {
        let mut app = test_app();
        app.on_core_event(Event::TurnStarted);
        app.handle.submit("fix the failing test"); // pushes to core, not shown yet
        app.blocks.push(Block::User("fix the failing test".into()));
        app.on_core_event(Event::TokenDelta("Reading the repo.".into()));
        app.on_core_event(Event::ToolCallStarted {
            name: "read_file".into(),
            preview: r#"{"path":"src/lib.rs"}"#.into(),
        });
        app.on_core_event(Event::ToolCallFinished {
            name: "read_file".into(),
            ok: true,
            duration_ms: 3,
            summary: "read_file: src/lib.rs (lines 1–10)".into(),
        });
        app.on_core_event(Event::UsageUpdated {
            prompt_tokens: 900,
            completion_tokens: 100,
        });

        // Phase 1: transcript + status before any modal overlays it.
        let screen = draw(&app);
        assert!(screen.contains("you ❯ fix the failing test"), "{screen}");
        assert!(screen.contains("Reading the repo."), "{screen}");
        assert!(screen.contains("read_file"), "{screen}");
        assert!(screen.contains("test-model-x"), "{screen}"); // status bar
        assert!(screen.contains("1000 tok"), "{screen}"); // usage meter

        // Phase 2: modal overlays the middle of the screen.
        app.on_core_event(Event::ApprovalRequired {
            id: 7,
            tool: "write_file".into(),
            input_preview: r#"{"path":"src/lib.rs"}"#.into(),
            suggested_rule: None,
            detail_preview: Some(
                "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,2 @@\n-fn old() {}\n+fn new() {}\n"
                    .into(),
            ),
            can_persist: true,
        });
        let screen = draw(&app);
        assert!(screen.contains("approval required"), "{screen}");
        assert!(screen.contains("fn new()"), "{screen}"); // syntax-highlighted diff body
        assert!(screen.contains("always prefix"), "{screen}");

        // Answering the modal clears it.
        if let Some(p) = app.pending.take() {
            assert_eq!(p.id, 7);
        }
        let screen2 = draw(&app);
        assert!(!screen2.contains("approval required"), "{screen2}");
    }

    #[tokio::test]
    async fn history_navigation_roundtrip() {
        let mut app = test_app();
        app.history.push_back("first task".into());
        app.history.push_back("second task".into());
        app.history_prev();
        assert_eq!(app.input, "second task");
        app.history_prev();
        assert_eq!(app.input, "first task");
        app.history_next();
        assert_eq!(app.input, "second task");
        app.history_next();
        assert_eq!(app.input, "");
    }
}
