use super::App;

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
    /// Parsed shell command when tool == bash (drives rule suggestions).
    pub bash_command: Option<String>,
}

impl App {
    pub(crate) fn history_prev(&mut self) {
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

    pub(crate) fn history_next(&mut self) {
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

    pub(crate) fn scroll_up_by(&mut self, n: u16) {
        self.scroll_from_bottom = self.scroll_from_bottom.saturating_add(n.max(1));
    }

    pub(crate) fn scroll_down_by(&mut self, n: u16) {
        self.scroll_from_bottom = self.scroll_from_bottom.saturating_sub(n.max(1));
    }
}
