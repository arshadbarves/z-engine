//! Terminal face: owns an [`AgentHandle`], consumes crossterm input +
//! core events, redraws. **All logic lives in z-engine-core** — this crate
//! only renders and translates keystrokes into commands.

mod app;
mod cassette;
mod cli;
mod headless;
mod views;

use std::path::PathBuf;
use std::sync::Arc;

use cli::{Args, USAGE};
use z_engine_core::agent::{LoopConfig, ResumeState, spawn_with_recorder, spawn_with_run_recorder};
use z_engine_core::config::{
    CliOverrides, Config, resolve_api_key, session_search_dirs, sessions_dir,
};
use z_engine_core::session::{self, SessionWriter};

fn resolve_session_file(id: &str) -> Option<PathBuf> {
    let direct = PathBuf::from(id);
    if direct.exists() {
        return Some(direct);
    }
    for dir in session_search_dirs() {
        let in_dir = dir.join(id);
        if in_dir.exists() {
            return Some(in_dir);
        }
        let with_ext = dir.join(format!("{id}.jsonl"));
        if with_ext.exists() {
            return Some(with_ext);
        }
    }
    None
}

fn init_logging() {
    let path = z_engine_core::config::app_data_write_dir();
    let _ = std::fs::create_dir_all(&path);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.join("zengine.log"));
    if let Ok(file) = file {
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
        let _ = tracing_subscriber::fmt()
            .with_writer(std::sync::Mutex::new(file))
            .with_env_filter(filter)
            .without_time()
            .try_init();
    }
}

async fn load_config_and_key(
    args: &Args,
    project_root: Option<&std::path::Path>,
) -> anyhow::Result<(Config, Option<String>)> {
    let cfg = Config::load(
        &CliOverrides {
            model: args.model.clone(),
            base_url: args.base_url.clone(),
        },
        project_root,
    )?;
    // A replayed run is served from its cassette; it must not so much as
    // read a credential, let alone send one.
    let key = if args.needs_api_key() {
        resolve_api_key()
    } else {
        None
    };
    Ok((cfg, key))
}

fn main() -> anyhow::Result<()> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match cli::parse(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("zengine: {e}");
            std::process::exit(2);
        }
    };
    if args.help {
        println!("zengine v{}\n\n{USAGE}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    init_logging();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run(args))
}

async fn run(args: Args) -> anyhow::Result<()> {
    let _ = z_engine_core::config::ensure_user_config();
    let project_root = args
        .project
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    if let Err(e) = args.check_tape_paths(&project_root) {
        eprintln!("zengine: {e}");
        std::process::exit(2);
    }
    let cfg = load_config_and_key(&args, Some(project_root.as_path())).await?;
    let (config, api_key) = cfg;

    // ---- session persistence (spec section 8) ------------------------
    let mut resume_state = None;
    let mut recorder = None;
    let mut session_tag: Option<String> = None;
    if args.resume || args.session.is_some() {
        let chosen = match &args.session {
            Some(id) => resolve_session_file(id),
            None => views::picker::pick_interactive()?,
        };
        if let Some(path) = chosen {
            let events = session::read_events(&path)?;
            let replayed = session::replay(&events);
            resume_state = Some(ResumeState {
                working: replayed.working,
                note_payloads: replayed.notes_replayed,
            });
            recorder = Some(SessionWriter::append_to(&path)?);
            session_tag = path
                .file_stem()
                .map(|s| s.to_string_lossy().chars().take(6).collect());
        }
    }
    if recorder.is_none() {
        let w = SessionWriter::create(&sessions_dir())?;
        session_tag = w
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().chars().take(6).collect());
        recorder = Some(w);
    }
    let session_tag = session_tag.unwrap_or_else(|| "??????".into());

    let lc = LoopConfig {
        model: config.model.clone(),
        base_url: config.base_url.clone(),
        api_key: api_key.clone(),
        project_root: project_root.clone(),
        tmp_dir: std::env::temp_dir(),
        initial_allow_rules: config.permissions.allow.clone(),
        max_context_tokens: config.max_context_tokens,
        max_output_tokens: config.max_output_tokens,
        hooks: config.hooks.clone(),
        compact_at_percent: config.compact_at_percent,
        keep_recent_messages: 12,
        review_enabled: config.review_enabled,
        mcp_servers: config.mcp_servers.clone(),
        auto_allow_tools: vec![],
        initial_mode: parse_mode(args.permission_mode.as_deref()),
        // Evidence-gated mode: opt-in per run, interactive or headless.
        guarded: args.guarded,
    };

    if args.headless_task.is_some() {
        return run_headless(args, lc, resume_state, recorder).await;
    }

    match api_key.as_deref() {
        Some(k) => {
            let tail: String = k.chars().rev().take(4).collect();
            tracing::info!(key_tail = %tail, "auth resolved");
            if config.base_url.contains("openrouter.ai") && !k.starts_with("sk-or-") {
                eprintln!(
                    "note: key does not look like an OpenRouter key (expected prefix sk-or-)"
                );
            }
        }
        None => {
            if !config.base_url.contains("localhost") && !config.base_url.contains("127.0.0.1") {
                eprintln!(
                    "warning: no API key found (set it in the GUI Settings page, \
                     or ZENGINE_API_KEY / ~/.config/z-engine/auth.json); provider calls will fail"
                );
            }
        }
    }

    let initial_mode = parse_mode(args.permission_mode.as_deref());
    use crossterm::ExecutableCommand;
    use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    // From this point every exit path must restore the terminal. A Drop
    // guard covers panics and early `?` returns alike — without it a
    // crash strands the user's shell in raw mode / alternate screen.
    struct TermGuard;
    impl Drop for TermGuard {
        fn drop(&mut self) {
            use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
            let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
        }
    }
    let _term_guard = TermGuard;

    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    let mut terminal = ratatui::Terminal::new(backend)?;

    let (handle, ev_rx) = spawn_with_recorder(lc, resume_state, recorder);
    app::run(
        &mut terminal,
        handle,
        ev_rx,
        config.clone(),
        &project_root,
        session_tag,
        initial_mode,
    )
    .await
}

/// One-shot mode, optionally taped. A recorded, replayed and plain run
/// are driven by the same runner; only the provider underneath differs.
async fn run_headless(
    args: Args,
    lc: LoopConfig,
    resume_state: Option<ResumeState>,
    recorder: Option<SessionWriter>,
) -> anyhow::Result<()> {
    let task = args.headless_task.clone().unwrap_or_default();
    let task = if task.trim().is_empty() {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        task
    };

    let taped = match (&args.record_run, &args.replay_run) {
        (Some(path), _) => Some(cassette::recording(path, &lc.base_url, lc.api_key.clone())?),
        (_, Some(path)) => {
            let taped = cassette::replaying(path)?;
            eprintln!(
                "zengine --replay-run · taping this run to {}",
                taped.tape.display()
            );
            Some(taped)
        }
        _ => None,
    };

    let outcome = match &taped {
        Some(taped) => {
            let (handle, ev_rx) = spawn_with_run_recorder(
                lc,
                Arc::clone(&taped.provider),
                resume_state,
                recorder,
                Some(Arc::clone(&taped.recorder)),
            );
            headless::run_one_shot(handle, ev_rx, &task, args.auto_approve).await
        }
        None => {
            let (handle, ev_rx) = spawn_with_recorder(lc, resume_state, recorder);
            headless::run_one_shot(handle, ev_rx, &task, args.auto_approve).await
        }
    };

    // How the run went is worth keeping whether or not it got where it was
    // going, so metrics are written before its verdict is returned.
    if let (Some(destination), Some(taped)) = (&args.metrics_out, &taped) {
        taped.recorder.settle().await;
        cassette::write_metrics(&taped.tape, destination)?;
    }
    outcome
}

fn parse_mode(s: Option<&str>) -> z_engine_core::agent::PermissionMode {
    match s {
        Some("accept-edits") => z_engine_core::agent::PermissionMode::AutoAcceptEdits,
        Some("plan") => z_engine_core::agent::PermissionMode::Plan,
        _ => z_engine_core::agent::PermissionMode::Normal,
    }
}
