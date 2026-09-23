//! The permission policy of a session, built from its effective settings.
//! A rebuild (settings reload) keeps the rules granted during the session.

use std::path::Path;

use z_engine_policy::{Policy, PolicyConfig, PolicyContext, PolicyError};

use super::effective::SessionSettings;

pub(crate) fn build_policy(
    settings: &SessionSettings,
    root: &Path,
    home: Option<&Path>,
    session_rules: &[String],
) -> (Policy, Vec<PolicyError>) {
    let permissions = &settings.settings.permissions;
    let config = PolicyConfig {
        allow: permissions.allow.clone(),
        ask: permissions.ask.clone(),
        deny: permissions.deny.clone(),
        auto_allow_read_only_bash: permissions.auto_allow_read_only_bash,
    };
    let context = PolicyContext {
        project_root: root.to_path_buf(),
        additional_dirs: settings.additional_dirs.clone(),
        home: home.map(Path::to_path_buf),
    };
    let (mut policy, mut errors) = Policy::new(&config, context);
    for rule in session_rules {
        if let Err(error) = policy.add_session_rule(rule) {
            errors.push(error);
        }
    }
    (policy, errors)
}
