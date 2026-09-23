//! Applying changed settings to a live session: settings, extensions (and
//! with them the agent types in the `Agent` tool), instructions, the model
//! client, the policy (session grants kept), the git snapshot, the context
//! window, MCP and language servers, the checks, and the repository map.

use std::path::Path;
use std::sync::Arc;

use z_engine_context::GitInfo;
use z_engine_host::summary;
use z_engine_protocol::NoticeLevel;

use super::snapshot::emit_snapshot;
use super::tools::rebuild_tools;
use crate::mcp::sync_servers;
use crate::orchestration::AgentRegistry;
use crate::session::SessionCore;
use crate::settings::{build_policy, load_session_settings, models, session_client};
use crate::sync::{lock, write};
use crate::verify::{discover_checks, publish_outcome};

pub(crate) async fn reload(core: &Arc<SessionCore>) {
    let shared = &core.shared;
    let report = load_session_settings(&shared.paths, &core.root, &shared.env);
    for (level, text) in &report.notices {
        core.events.notice(*level, text.clone());
    }
    let settings = Arc::new(report.settings);
    let (client, warning) =
        session_client(&shared.paths, &settings.settings, shared.factory.as_ref());
    if let Some(warning) = warning {
        core.events.notice(NoticeLevel::Warn, warning);
    }
    let granted = lock(&core.policy).session_rules();
    let home = shared.paths.home_dir.as_deref();
    let (policy, errors) = build_policy(&settings, &core.root, home, &granted);
    for error in errors {
        core.events.notice(
            NoticeLevel::Warn,
            format!("ignored permission rule: {error}"),
        );
    }
    let git = git_info(&core.root).await;
    let catalog = core.catalog();
    let limit = models::context_window(&settings.settings, catalog.as_deref(), &core.main_model());
    let checks = discover_checks(&core.root, &settings.settings.verification.checks).await;
    core.agents
        .set_registry(AgentRegistry::build(&settings.extensions.agents));
    if let Some(replaced) = core.lsp.configure(&core.root, &settings.settings.lsp) {
        tokio::spawn(async move { replaced.shutdown().await });
    }
    *write(&core.settings) = settings;
    rebuild_tools(core);
    *write(&core.client) = client;
    *lock(&core.policy) = policy;
    *lock(&core.git) = git;
    core.checks.set(checks);
    core.repo_map.invalidate();
    core.with_state(|state| state.context_limit = limit);
    sync_servers(core);
    emit_snapshot(core);
    publish_outcome(core).await;
}

/// The environment section's git snapshot; `None` outside a repository.
pub(crate) async fn git_info(root: &Path) -> Option<GitInfo> {
    let summary = summary(root).await?;
    Some(GitInfo {
        branch: summary
            .branch
            .unwrap_or_else(|| "(detached HEAD)".to_string()),
        status_short: summary.status_short,
        recent_commits: summary.recent_commits,
    })
}
