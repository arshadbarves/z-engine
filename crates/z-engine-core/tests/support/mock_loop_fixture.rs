use std::time::Duration;

use z_engine_core::agent::{Event, LoopConfig};

pub(crate) fn cfg_for(base_url: String, project_root: &std::path::Path) -> LoopConfig {
    LoopConfig {
        model: "test-model".into(),
        base_url,
        api_key: Some("test-key-not-real".into()),
        project_root: project_root.to_path_buf(),
        tmp_dir: project_root.join("tmp-out"),
        initial_allow_rules: vec!["echo*".to_string()],
        max_context_tokens: 100_000,
        max_output_tokens: 16_384,
        max_task_continuations: 0,
        hooks: Default::default(),
        compact_at_percent: 92,
        keep_recent_messages: 12,
        review_enabled: false,
        mcp_servers: vec![],
        auto_allow_tools: vec![],
        initial_mode: z_engine_core::agent::PermissionMode::Normal,
        shell_path: None,
    }
}

/// Drain events until `pred` matches or a deadline passes.
pub(crate) async fn wait_for(
    ev: &mut z_engine_core::agent::EventRx,
    pred: impl Fn(&Event) -> bool,
) -> Event {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for event"
        );
        let e = tokio::time::timeout(Duration::from_millis(500), ev.recv())
            .await
            .ok()
            .flatten();
        match e {
            Some(e) if pred(&e) => return e,
            Some(_) => continue,
            None => continue,
        }
    }
}
