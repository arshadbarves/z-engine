//! Built-in slash commands handled without the model. Prompt commands,
//! custom commands and skills arrive with the commands phase; anything
//! else is answered with a notice.

use std::sync::Arc;

use z_engine_protocol::{Event, NoticeLevel};

use super::remember::remember;
use super::reports;
use crate::session::SessionCore;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Slash {
    /// The actor runs a manual compaction with these instructions.
    Compact(Option<String>),
    Handled,
}

pub(crate) async fn run_command(core: &Arc<SessionCore>, name: &str, args: &str) -> Slash {
    let name = name.trim().trim_start_matches('/');
    match name {
        "compact" => {
            let instructions = Some(args.trim().to_string()).filter(|text| !text.is_empty());
            return Slash::Compact(instructions);
        }
        "context" => output(core, "context", reports::context(core)),
        "cost" => output(core, "cost", reports::cost(core)),
        "status" => output(core, "status", reports::status(core)),
        "remember" => match remember(core, args).await {
            Ok((path, bullet)) => output(
                core,
                "remember",
                format!("Saved to `{}`:\n\n{bullet}", path.display()),
            ),
            Err(error) => core.events.notice(NoticeLevel::Warn, error.to_string()),
        },
        other => core.events.notice(
            NoticeLevel::Info,
            format!("/{other} is not available in this version yet."),
        ),
    }
    Slash::Handled
}

fn output(core: &SessionCore, name: &str, markdown: String) {
    core.events.emit(Event::CommandOutput {
        name: name.to_string(),
        markdown,
    });
}
