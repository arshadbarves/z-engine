use z_engine_core::agent::Event;

use super::{App, Block, PendingApproval};

impl App {
    /// Close an open thinking block with a collapsed summary line.
    fn close_thinking(&mut self) {
        if self.thinking_open {
            self.blocks.push(Block::Notice(format!(
                "✻ thinking collapsed ({} chars)",
                self.thinking_chars
            )));
            self.thinking_open = false;
            self.thinking_chars = 0;
        }
    }

    // ---- core events ------------------------------------------------------

    pub(crate) fn on_core_event(&mut self, ev: Event) {
        match ev {
            Event::TurnStarted => {
                self.finish_streaming();
            }
            Event::ReasoningDelta(r) => {
                if !self.thinking_open {
                    self.blocks.push(Block::Notice("✻ thinking…".into()));
                    self.thinking_open = true;
                    self.thinking_chars = 0;
                }
                self.thinking_chars += r.chars().count() as u64;
            }
            Event::ToolOutputDelta { tool_name: _, text } => {
                // Live tool output appended to the running tool card
                if let Some(Block::ToolCall { preview, .. }) = self.blocks.last_mut() {
                    preview.push('\n');
                    preview.push_str(&text);
                }
            }
            Event::TokenDelta(t) => {
                self.close_thinking();
                self.assistant_streaming_block().push_str(&t);
            }
            Event::ToolCallStarted { name, preview } => {
                self.close_thinking();
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
                bash_command,
            } => {
                self.finish_streaming();
                self.pending = Some(PendingApproval {
                    id,
                    tool,
                    input_preview,
                    suggested_rule,
                    detail_preview,
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
            Event::StatusNote(s) => self.blocks.push(Block::Notice(s)),
            Event::TurnCompleted { .. } => {
                self.finish_streaming();
                self.blocks.push(Block::Notice("✓ done".into()));
                self.turn_active = false;
            }
            Event::TurnAborted => {
                self.close_thinking();
                self.finish_streaming();
                self.blocks.push(Block::Notice("■ aborted".into()));
                self.turn_active = false;
            }
            Event::Error(msg) => {
                self.close_thinking();
                self.finish_streaming();
                self.blocks.push(Block::Error(msg));
                self.turn_active = false;
            }
            Event::TranscriptTrimmed { keep_turn } => {
                self.close_thinking();
                self.finish_streaming();
                let mut seen = 0u64;
                let mut cut = self.blocks.len();
                for (i, b) in self.blocks.iter().enumerate() {
                    if matches!(b, Block::User(_)) {
                        if seen == keep_turn {
                            cut = i;
                            break;
                        }
                        seen += 1;
                    }
                }
                if let Some(Block::User(t)) = self.blocks.get(cut) {
                    self.input = t.clone();
                }
                self.blocks.truncate(cut);
                self.turn_active = false;
                self.pending = None;
            }
            Event::SessionTitle { .. } => {}
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
