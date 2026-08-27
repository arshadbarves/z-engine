use crossterm::event::{
    Event as CtEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use harness_core::agent::{ApprovalDecision, PermissionMode};

use super::{App, Block, PendingApproval};

impl App {
    // ---- input handling ---------------------------------------------------

    pub(crate) fn on_ct_event(&mut self, area_height: u16, ev: CtEvent) {
        match ev {
            CtEvent::Key(k) => self.on_key(area_height, k),
            CtEvent::Mouse(m) => match m.kind {
                // v1.1 fix: wheel-up reveals older content (increase offset).
                MouseEventKind::ScrollUp => self.scroll_up_by(area_height / 2),
                MouseEventKind::ScrollDown => self.scroll_down_by(area_height / 2),
                _ => {}
            },
            _ => {}
        }
    }

    pub(crate) fn on_key(&mut self, area_height: u16, k: KeyEvent) {
        if k.kind == KeyEventKind::Release {
            return;
        }

        // Approval modal swallows keys while visible.
        if let Some(pending) = self.pending.take() {
            let ctrl_c = k.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('C'));
            let rule_for = |p: &PendingApproval| {
                p.bash_command
                    .as_deref()
                    .map(harness_core::perms::PolicyEngine::suggested_rule)
                    .or_else(|| p.suggested_rule.clone())
                    .unwrap_or_else(|| "bash*".into())
            };
            match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('1') => {
                    self.handle.approve(pending.id, ApprovalDecision::Once);
                    self.blocks.push(Block::Notice(format!(
                        "\u{2713} approved once: {} {}",
                        pending.tool, pending.input_preview
                    )));
                }
                KeyCode::Char('a')
                | KeyCode::Char('A')
                | KeyCode::Char('s')
                | KeyCode::Char('S')
                | KeyCode::Char('2') => {
                    let rule = rule_for(&pending);
                    self.handle.approve(
                        pending.id,
                        ApprovalDecision::AlwaysSession { rule: rule.clone() },
                    );
                    self.blocks.push(Block::Notice(format!(
                        "\u{2713} always this prefix [{rule}] (session)"
                    )));
                }
                KeyCode::Char('p') | KeyCode::Char('P') | KeyCode::Char('3')
                    if pending.can_persist =>
                {
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
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('4') | KeyCode::Esc => {
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
                    "/help" => {
                        self.input.clear();
                        self.blocks.push(Block::Notice(
                            "commands: /help /clear /compact /notes /cost /status /quit\nkeys: Esc abort · Shift+Tab mode · Ctrl-C twice quit".into(),
                        ));
                        return;
                    }
                    "/clear" => {
                        self.input.clear();
                        self.handle.shutdown();
                        self.should_quit = true;
                        self.blocks.push(Block::Notice(
                            "[context cleared — restart for a fresh session]".into(),
                        ));
                        return;
                    }
                    "/cost" => {
                        self.input.clear();
                        let total = self.prompt_tokens + self.completion_tokens;
                        self.blocks.push(Block::Notice(format!(
                            "tokens this session: prompt={} completion={} total={}",
                            self.prompt_tokens, self.completion_tokens, total
                        )));
                        return;
                    }
                    "/status" => {
                        self.input.clear();
                        self.blocks.push(Block::Notice(format!(
                            "model={} \u{b7} mode={} \u{b7} session={} \u{b7} tokens {}/{}",
                            self.model,
                            self.ui_mode.label(),
                            self.session_tag,
                            self.prompt_tokens + self.completion_tokens,
                            self.max_context_tokens
                        )));
                        return;
                    }
                    "/quit" | "/exit" => {
                        self.should_quit = true;
                        self.handle.shutdown();
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
            KeyCode::BackTab | KeyCode::Tab if k.modifiers.contains(KeyModifiers::SHIFT) => {
                let next = match self.ui_mode {
                    PermissionMode::Normal => PermissionMode::AutoAcceptEdits,
                    PermissionMode::AutoAcceptEdits => PermissionMode::Plan,
                    PermissionMode::Plan => PermissionMode::Normal,
                };
                self.ui_mode = next;
                self.handle.set_mode(next);
                self.blocks
                    .push(Block::Notice(format!("[mode: {}]", next.label())));
            }
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.push(c);
            }
            _ => {}
        }
    }
}
