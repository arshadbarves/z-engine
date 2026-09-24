//! `Notification` hooks: the session is waiting on the user.

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::event::{HookEvent, HookInput};
use super::runner::run_hooks;
use crate::session::SessionCore;

pub(crate) async fn notify(core: &SessionCore, message: &str, cancel: &CancellationToken) {
    let input = HookInput::new().with("message", json!(message));
    run_hooks(
        &core.hook_env(),
        &core.events,
        HookEvent::Notification,
        input,
        cancel,
    )
    .await;
}
