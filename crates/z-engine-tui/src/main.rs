//! Terminal face: owns an [`AgentHandle`], consumes crossterm input +
//! core events, redraws. **All logic lives in z-engine-core** — this crate
//! only renders and translates keystrokes into commands.

mod app;
mod headless;
mod views;

use std::path::PathBuf;

use z_engine_core::agent::{LoopConfig, ResumeState, spawn_with_recorder};
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

/// CLI surface for v0.1 (formalized in v1.0).
#[derive(Debug, Default)]
struct Args {
    model: Option<String>,
    base_url: Option<String>,
    project: Option<PathBuf>,
    /// One-shot mode: read task from argv/stdin, stream plain-text events,
    /// exit non-zero on failure. Developer/acceptance flag (spec §9 v1.0
    /// formalizes it; recorded in docs/deviations.md).
    headless_task: Option<String>,
    /// Headless companion: auto-approve every gated action (unsafe
    /// convenience for scripted acceptance runs).
    auto_approve: bool,
    /// Starting permission mode (default|accept-edits|plan).
    permission_mode: Option<String>,
    /// Open the session picker at startup.
    resume: bool,
    /// Resume a specific session by ULID (or path).
    session: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0usize;
    while i < argv.len() {
        let need_value = |i: &mut usize, flag: &str| -> Result<String, String> {
            *i += 1;
            argv.get(*i).cloned().ok_or(format!("{flag} needs a value"))
        };
        match argv[i].as_str() {
            "--model" => args.model = Some(need_value(&mut i, "--model")?),
            "--base-url" => args.base_url = Some(need_value(&mut i, "--base-url")?),
            "--project" => args.project = Some(PathBuf::from(need_value(&mut i, "--project")?)),
            "--headless" => {
                // Task = following words up to the next --flag; none ⇒ stdin.
                let mut parts: Vec<String> = Vec::new();
                while i + 1 < argv.len() && !argv[i + 1].starts_with('-') {
                    i += 1;
                    parts.push(argv[i].clone());
                }
                args.headless_task = Some(if parts.is_empty() {
                    String::new()
                } else {
                    parts.join(" ")
                });
            }
            "--auto-approve" => args.auto_approve = true,
            "--resume" => args.resume = true,
            "--permission-mode" => {
                args.permission_mode = Some(need_value(&mut i, "--permission-mode")?)
            }
            "--session" => args.session = Some(need_value(&mut i, "--session")?),
            "--help" | "-h" => {
                println!(
                    "zengine v{} - personal TUI coding agent\n\nUSAGE:\n  zengine [--model M] [--base-url URL] [--project DIR]\n          [--resume | --session ULID]\n          [--headless \"task\" | --headless < task.txt] [--auto-approve]",
                    env!("CARGO_PKG_VERSION")
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    Ok(args)
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
    let key = resolve_api_key();
    Ok((cfg, key))
}

fn main() -> anyhow::Result<()> {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("zengine: {e}");
            std::process::exit(2);
        }
    };
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
        api_key,
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
        // Guarded (evidence-gated) mode is opt-in and not yet exposed here.
        guarded: false,
    };

    if let Some(task) = args.headless_task {
        let task = if task.trim().is_empty() {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        } else {
            task
        };
        let (handle, ev_rx) = spawn_with_recorder(lc, resume_state, recorder);
        return headless::run_one_shot(handle, ev_rx, &task, args.auto_approve).await;
    }

    match resolve_api_key() {
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

fn parse_mode(s: Option<&str>) -> z_engine_core::agent::PermissionMode {
    match s {
        Some("accept-edits") => z_engine_core::agent::PermissionMode::AutoAcceptEdits,
        Some("plan") => z_engine_core::agent::PermissionMode::Plan,
        _ => z_engine_core::agent::PermissionMode::Normal,
    }
}
