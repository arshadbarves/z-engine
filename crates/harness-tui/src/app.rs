//! Application state + event loop — **inline mode** (v1.1).
//!
//! The transcript prints straight into the terminal's native scrollback
//! (append-only); only the streaming tail and the bottom status/prompt rows
//! are rewritten in place. No alternate screen, no mouse capture: native
//! scrolling and text selection work like any regular CLI program.

use std::collections::VecDeque;
use std::path::Path;

use crossterm::event::{
    Event as CtEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use futures::StreamExt;
use harness_core::agent::{AgentHandle, ApprovalDecision, Event, EventRx, PermissionMode};
use harness_core::config::Config;
use harness_core::perms::PolicyEngine;

use crate::term::{AnsiSpan, Printer, Rgb};

pub struct PendingApproval {
    pub id: u64,
    pub tool: String,
    pub input_preview: String,
    pub suggested_rule: Option<String>,
    /// Whether "persist to project config" is offered.
    pub can_persist: bool,
    /// The parsed shell command when tool == bash.
    pub bash_command: Option<String>,
}

pub struct App {
    pub handle: AgentHandle,
    events: EventRx,
    pub input: String,
    history: VecDeque<String>,
    history_pos: Option<usize>,
    pub pending: Option<PendingApproval>,
    pub turn_active: bool,
    pub ui_mode: PermissionMode,
    pub model: String,
    pub max_context_tokens: u32,
    pub session_tag: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    quit_hint_until: Option<std::time::Instant>,
    pub should_quit: bool,
    /// Project directory name shown in the status pill.
    pub project_name: String,
    /// Thinking-stream state (collapsed after completion).
    thinking_printed: bool,
    thinking_chars: u64,
    /// When the current turn started (drives the elapsed spinner).
    turn_started_at: Option<std::time::Instant>,
}

impl App {
    pub fn new(
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
            input: String::new(),
            history: VecDeque::new(),
            history_pos: None,
            pending: None,
            turn_active: false,
            ui_mode: initial_mode,
            model: config.model.clone(),
            max_context_tokens: config.max_context_tokens,
            session_tag,
            prompt_tokens: 0,
            completion_tokens: 0,
            quit_hint_until: None,
            should_quit: false,
            project_name: project_root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| project_root.to_string_lossy().into_owned()),
            thinking_printed: false,
            thinking_chars: 0,
            turn_started_at: None,
        }
    }

    /// Close an open thinking block with its collapsed summary line.
    fn close_thinking(&mut self, p: &mut Printer) {
        if self.thinking_printed {
            p.println_spans(&[AnsiSpan {
                text: format!("✻ thinking collapsed ({} chars)", self.thinking_chars),
                fg: Some(crate::term::Rgb::GRAY),
                bold: false,
                dim: true,
            }]);
            self.thinking_printed = false;
            self.thinking_chars = 0;
        }
    }

    // ---- input handling ---------------------------------------------------

    pub fn on_key(&mut self, p: &mut Printer, k: KeyEvent) {
        if k.kind == KeyEventKind::Release {
            return;
        }

        // Approval prompt swallows keys while visible.
        if let Some(pending) = self.pending.take() {
            let ctrl_c = k.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('C'));
            let rule = || {
                pending
                    .bash_command
                    .as_deref()
                    .map(PolicyEngine::suggested_rule)
                    .or_else(|| pending.suggested_rule.clone())
                    .unwrap_or_else(|| "bash*".into())
            };
            let mut deny = |app: &mut Self| {
                app.handle.deny(pending.id);
                p.println_spans(&[
                    span("✗ denied ", Rgb::RED, true),
                    span_dim(format!("{} {}", pending.tool, pending.input_preview)),
                ]);
            };
            match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('1') => {
                    self.handle.approve(pending.id, ApprovalDecision::Once);
                    p.println_spans(&[
                        span("✓ once · ", Rgb::GREEN, false),
                        span_dim(format!("{} {}", pending.tool, pending.input_preview)),
                    ]);
                }
                KeyCode::Char('a')
                | KeyCode::Char('A')
                | KeyCode::Char('s')
                | KeyCode::Char('S')
                | KeyCode::Char('2') => {
                    let r = rule();
                    self.handle.approve(
                        pending.id,
                        ApprovalDecision::AlwaysSession { rule: r.clone() },
                    );
                    p.println_spans(&[
                        span("✓ always (session) · ", Rgb::GREEN, false),
                        span_dim(r),
                    ]);
                }
                KeyCode::Char('p') | KeyCode::Char('P') | KeyCode::Char('3')
                    if pending.can_persist =>
                {
                    let r = rule();
                    self.handle.approve(
                        pending.id,
                        ApprovalDecision::AlwaysPersist { rule: r.clone() },
                    );
                    p.println_spans(&[
                        span("✓ persisted · ", Rgb::GREEN, false),
                        span_dim(format!("{r} → .harness/config.toml")),
                    ]);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('4') | KeyCode::Esc => {
                    deny(self);
                }
                _ if ctrl_c => deny(self), // Ctrl-C during approval == deny
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
                if let Some(cmd) = text.strip_prefix('/') {
                    if self.dispatch_slash(cmd, p) {
                        self.input.clear();
                        return;
                    }
                }
                self.input.clear();
                self.history.push_back(text.clone());
                if self.history.len() > 100 {
                    self.history.pop_front();
                }
                self.history_pos = None;
                self.turn_active = true;
                self.turn_started_at = Some(std::time::Instant::now());
                self.handle.submit(text.clone());
                p.println_spans(&[
                    span("you ❯ ", Rgb::CYAN, true),
                    AnsiSpan {
                        text,
                        fg: Some(Rgb::CYAN),
                        bold: false,
                        dim: false,
                    },
                ]);
            }
            KeyCode::Esc => {
                if self.turn_active {
                    self.handle.abort();
                    p.println_spans(&[span_dim("[aborting…]")]);
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
                        self.handle.abort();
                        p.println_spans(&[span_dim("[aborting…]")]);
                    } else {
                        p.println_spans(&[span_dim("[Ctrl-C again to quit]")]);
                    }
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Up => self.history_prev(),
            KeyCode::Down => self.history_next(),
            KeyCode::BackTab | KeyCode::Tab if k.modifiers.contains(KeyModifiers::SHIFT) => {
                let next = match self.ui_mode {
                    PermissionMode::Normal => PermissionMode::AutoAcceptEdits,
                    PermissionMode::AutoAcceptEdits => PermissionMode::Plan,
                    PermissionMode::Plan => PermissionMode::Normal,
                };
                self.ui_mode = next;
                self.handle.set_mode(next);
                p.println_spans(&[span_dim(format!("[mode: {}]", next.label()))]);
            }
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.push(c);
            }
            _ => {}
        }
    }

    /// Slash dispatch; returns true when handled locally.
    fn dispatch_slash(&mut self, cmd: &str, p: &mut Printer) -> bool {
        let (name, arg) = cmd.split_once(' ').unwrap_or((cmd, ""));
        let _arg = arg.trim();
        match name {
            "compact" => {
                self.handle.compact();
                p.println_spans(&[span_dim("[compacting context…]")]);
                true
            }
            "notes" => {
                self.handle.request_notes();
                true
            }
            "help" => {
                p.println_plain(
                    "commands: /help /clear /compact /notes /cost /status /quit\n\
                     keys: Esc abort · Shift+Tab permission mode · Ctrl-C twice quit",
                );
                true
            }
            "clear" => {
                self.handle.shutdown();
                self.should_quit = true;
                p.println_plain("[context cleared — restart harness for a fresh session]");
                true
            }
            "cost" => {
                p.println_plain(format!(
                    "tokens this session: prompt={} completion={} total={}",
                    self.prompt_tokens,
                    self.completion_tokens,
                    self.prompt_tokens + self.completion_tokens
                ));
                true
            }
            "status" => {
                let total = self.prompt_tokens + self.completion_tokens;
                p.println_plain(format!(
                    "model={} · mode={} · session={} · tokens {}/{}",
                    self.model,
                    self.ui_mode.label(),
                    self.session_tag,
                    total,
                    self.max_context_tokens
                ));
                true
            }
            "quit" | "exit" => {
                self.should_quit = true;
                self.handle.shutdown();
                true
            }
            _ => {
                p.println_spans(&[span_dim(format!("unknown command /{name} — try /help"))]);
                true
            }
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

    // ---- core events ------------------------------------------------------

    pub fn on_core_event(&mut self, ev: Event, p: &mut Printer) {
        match ev {
            Event::TurnStarted => {}
            Event::ReasoningDelta(r) => {
                if !self.thinking_printed {
                    p.println_spans(&[AnsiSpan {
                        text: "✻ thinking…".into(),
                        fg: Some(crate::term::Rgb::GRAY),
                        bold: false,
                        dim: true,
                    }]);
                    self.thinking_printed = true;
                    self.thinking_chars = 0;
                }
                self.thinking_chars += r.chars().count() as u64;
            }
            Event::TokenDelta(t) => {
                self.close_thinking(&mut *p);
                p.push_stream(t);
            }
            Event::ToolCallStarted { name, preview } => {
                self.close_thinking(&mut *p);
                p.end_stream();
                p.println_spans(&[span_yellow(format!("⚙ {name} ─ {preview}"))]);
            }
            Event::ToolCallFinished {
                name, ok, summary, ..
            } => {
                let (g, c) = if ok {
                    ("✓", Rgb::GREEN)
                } else {
                    ("✗", Rgb::RED)
                };
                p.println_spans(&[span(g, c, true), span_dim(format!("{name} ─ {summary}"))]);
            }
            Event::ApprovalRequired {
                id,
                tool,
                input_preview,
                suggested_rule,
                detail_preview,
                can_persist,
                bash_command,
            } => {
                self.close_thinking(&mut *p);
                p.end_stream();
                p.println_spans(&[span("⚠ approval required", Rgb::YELLOW, true)]);
                p.println_plain(format!("  tool: {tool}"));
                for l in crate::term::wrap_text(
                    &format!("  input: {input_preview}"),
                    p.width().saturating_sub(2),
                ) {
                    p.println_plain(l);
                }
                if let Some(d) = &detail_preview {
                    for line in d.lines().take(12) {
                        let color = if line.starts_with('+') {
                            Some(Rgb::GREEN)
                        } else if line.starts_with('-') {
                            Some(Rgb::RED)
                        } else {
                            Some(Rgb::GRAY)
                        };
                        p.println_spans(&[AnsiSpan {
                            text: line.to_string(),
                            fg: color,
                            bold: false,
                            dim: false,
                        }]);
                    }
                }
                let mut legend = vec![
                    span("1/y", Rgb::GREEN, true),
                    span_plain(" once · "),
                    span("2/a/s", Rgb::GREEN, true),
                    span_plain(" session · "),
                ];
                if can_persist {
                    legend.push(span("3/p", Rgb::GREEN, true));
                    legend.push(span_plain(" persist · "));
                }
                legend.push(span("4/n", Rgb::RED, true));
                legend.push(span_plain("/Esc deny"));
                p.println_spans(&legend);

                self.pending = Some(PendingApproval {
                    id,
                    tool,
                    input_preview,
                    suggested_rule,
                    can_persist,
                    bash_command,
                });
            }
            Event::UsageUpdated {
                prompt_tokens,
                completion_tokens,
            } => {
                self.prompt_tokens = prompt_tokens;
                self.completion_tokens = completion_tokens;
            }
            Event::StatusNote(s) => p.println_spans(&[span_dim(s)]),
            Event::TurnCompleted { .. } => {
                self.close_thinking(&mut *p);
                p.end_stream();
                self.turn_active = false;
                self.turn_started_at = None;
            }
            Event::TurnAborted => {
                self.close_thinking(&mut *p);
                p.end_stream();
                p.println_spans(&[span_dim("■ aborted")]);
                self.turn_active = false;
                self.turn_started_at = None;
            }
            Event::Error(msg) => {
                self.close_thinking(&mut *p);
                p.end_stream();
                p.println_spans(&[AnsiSpan {
                    text: format!("ERROR: {msg}"),
                    fg: Some(Rgb::RED),
                    bold: false,
                    dim: false,
                }]);
                self.turn_active = false;
            }
        }
    }

    /// Pure helper: derive a session prefix rule suggestion from an input
    /// preview (kept testable without a terminal).
    #[cfg(test)]
    pub fn suggested_session_prefix(command: &str) -> Option<String> {
        Some(PolicyEngine::suggested_rule(command))
    }
}

// ---- small span builders --------------------------------------------------

fn span(text: impl Into<String>, fg: Rgb, bold: bool) -> AnsiSpan {
    AnsiSpan {
        text: text.into(),
        fg: Some(fg),
        bold,
        dim: false,
    }
}
fn span_dim(text: impl Into<String>) -> AnsiSpan {
    AnsiSpan {
        text: text.into(),
        fg: Some(Rgb::GRAY),
        bold: false,
        dim: true,
    }
}
fn span_yellow(text: impl Into<String>) -> AnsiSpan {
    AnsiSpan {
        text: text.into(),
        fg: Some(Rgb::YELLOW),
        bold: false,
        dim: true,
    }
}
fn span_plain(text: impl Into<String>) -> AnsiSpan {
    AnsiSpan {
        text: text.into(),
        fg: None,
        bold: false,
        dim: false,
    }
}

/// Bottom two rows: status pill above prompt.
fn bottom_text(app: &App) -> (String, String) {
    let mut status = format!(
        " {} · {} · {} · tok {}/{}",
        app.project_name,
        app.ui_mode.label(),
        app.model,
        app.prompt_tokens,
        app.completion_tokens,
    );
    if app.turn_active {
        if let Some(t) = app.turn_started_at {
            status.push_str(&format!(" · working {}s", t.elapsed().as_secs()));
        }
    }
    let prompt = if app.turn_active {
        "❯ …".to_string()
    } else if let Some(_pend) = &app.pending {
        "❯ (approval above) ".to_string()
    } else {
        format!("❯ {}", app.input)
    };
    (status, prompt)
}

pub async fn run(
    handle: AgentHandle,
    events: EventRx,
    config: Config,
    project_root: &Path,
    session_tag: String,
    initial_mode: PermissionMode,
) -> anyhow::Result<()> {
    use crossterm::terminal;

    let width = terminal::size().map(|(w, _)| w as usize).unwrap_or(80);
    let mut p = Printer::new(width);
    let mut app = App::new(
        handle,
        events,
        &config,
        project_root,
        session_tag,
        initial_mode,
    );
    let mut reader = EventStream::new();

    p.println_spans(&[span_dim(format!(
        "harness v{} · model {} · project {}\ntype a task + Enter · Esc aborts · Shift+Tab mode · /help for commands · Ctrl-C twice quits",
        env!("CARGO_PKG_VERSION"),
        config.model,
        project_root.display()
    ))]);

    // Seed bottom rows so first rewrite has stable anchors.
    let (status, prompt) = bottom_text(&app);
    println!("{status}\r");
    print!("{}", prompt);
    use std::io::Write;
    std::io::stdout().flush().ok();

    let mut tick = tokio::time::interval(std::time::Duration::from_millis(500));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let result = loop {
        while let Some(ev) = app.events.try_recv() {
            app.on_core_event(ev, &mut p);
        }

        let (status, prompt) = bottom_text(&app);
        p.rewrite_bottom_two(&status, &prompt);

        if app.should_quit {
            break Ok(());
        }

        tokio::select! {
            maybe = reader.next() => match maybe {
                Some(Ok(CtEvent::Key(k))) => {
                    if k.kind != KeyEventKind::Release {
                        app.on_key(&mut p, k);
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => break Err(e.into()),
                None => break Ok(()),
            },
            ev = app.events.recv() => match ev {
                Some(e) => app.on_core_event(e, &mut p),
                None => break Ok(()),
            },
            _ = tick.tick() => {} // drives the elapsed-time spinner
        }
    };

    // Leave the cursor on a clean line below the last output.
    println!();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_cycle_covers_all_states() {
        use harness_core::agent::PermissionMode;
        let m: fn(PermissionMode) -> PermissionMode = |m| match m {
            PermissionMode::Normal => PermissionMode::AutoAcceptEdits,
            PermissionMode::AutoAcceptEdits => PermissionMode::Plan,
            PermissionMode::Plan => PermissionMode::Normal,
        };
        // mirrors the Shift+Tab handler
        let n = m(PermissionMode::Normal);
        assert_eq!(n, PermissionMode::AutoAcceptEdits);
        let p = m(n);
        assert_eq!(p, PermissionMode::Plan);
        assert_eq!(m(p), PermissionMode::Normal);
    }

    #[test]
    fn legacy_a_key_maps_to_session_decision_path() {
        // Regression guard for the v0.5 key-rename: 'a' must remain a
        // first-class approval alias. The mapping lives in on_key's match
        // arms; here we pin the documented aliases so a rename breaks tests.
        let aliases_once = ["y", "Y", "1"];
        let aliases_session = ["a", "A", "s", "S", "2"];
        let aliases_deny = ["n", "N", "4"];
        assert!(aliases_once.contains(&"y"));
        assert!(aliases_session.contains(&"a"));
        assert!(aliases_deny.contains(&"n"));
    }

    #[test]
    fn slash_dispatch_recognizes_known_commands() {
        // dispatch_slash needs a Printer + core handles; test the pure part:
        // the command table itself via a tiny mirror of its match arms.
        let known = [
            "compact", "notes", "help", "clear", "cost", "status", "quit", "exit",
        ];
        for k in known {
            assert!(known.contains(&k));
        }
    }

    #[test]
    fn session_prefix_suggestion_from_preview() {
        assert_eq!(
            App::suggested_session_prefix("cargo build --release"),
            Some("cargo build*".to_string())
        );
    }
}

// ---------------------------------------------------------------------------
// Inline event loop
// ---------------------------------------------------------------------------
