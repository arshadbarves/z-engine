//! Application state + the single event loop (crossterm ⇄ core events).

mod input;
mod reducer;
mod run;
mod state;

pub use run::run;
pub use state::{Block, PendingApproval};

use std::collections::VecDeque;
use std::path::Path;

use z_engine_core::agent::{AgentHandle, EventRx, PermissionMode};
use z_engine_core::config::Config;

pub struct App {
    pub handle: AgentHandle,
    pub(crate) events: EventRx,
    pub blocks: Vec<Block>,
    pub input: String,
    pub(crate) history: VecDeque<String>,
    pub(crate) history_pos: Option<usize>,
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
    pub pricing: Option<z_engine_core::context::cost::Pricing>,
    pub(crate) quit_hint_until: Option<std::time::Instant>,
    pub should_quit: bool,
    /// Interaction permission mode (Shift+Tab cycles).
    pub ui_mode: PermissionMode,
    /// When the active turn started — drives the elapsed spinner.
    pub turn_started_at: Option<std::time::Instant>,
    /// Project directory name (status pill).
    pub project_name: String,
    /// Live reasoning ("thinking") stream state.
    pub(crate) thinking_open: bool,
    pub(crate) thinking_chars: u64,
}

impl App {
    pub(crate) fn new(
        handle: AgentHandle,
        events: EventRx,
        config: &Config,
        project_root: &Path,
        session_tag: String,
        initial_mode: PermissionMode,
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
            ui_mode: initial_mode,
            model: config.model.clone(),
            max_context_tokens: config.max_context_tokens,
            session_tag,
            prompt_tokens: 0,
            completion_tokens: 0,
            pricing: z_engine_core::context::cost::for_model(&config.model),
            quit_hint_until: None,
            should_quit: false,
            turn_started_at: None,
            project_name: project_root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| project_root.to_string_lossy().into_owned()),
            thinking_open: false,
            thinking_chars: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views;
    use ratatui::{Terminal, backend::TestBackend};
    use z_engine_core::agent::{Event, LoopConfig, spawn};

    fn test_app() -> App {
        // Bogus provider URL: handles are valid, no network is touched.
        let cfg = LoopConfig::new("test-model-x", "http://127.0.0.1:1/v1");
        let (handle, ev_rx) = spawn(cfg);
        let config = Config {
            model: "test-model-x".into(),
            base_url: "http://127.0.0.1:1/v1".into(),
            max_context_tokens: 120_000,
            max_output_tokens: 16_384,
            hooks: Default::default(),
            compact_at_percent: 92,
            permissions: Default::default(),
            review_enabled: true,
            mcp_servers: vec![],
            cost_overrides: Default::default(),
            shell_path: None,
        };
        App::new(
            handle,
            ev_rx,
            &config,
            Path::new("/tmp"),
            "tst123".to_string(),
            PermissionMode::Normal,
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
            bash_command: None,
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
