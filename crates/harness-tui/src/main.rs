//! Terminal face: owns an [`AgentHandle`], consumes crossterm input +
//! core events, redraws. **All logic lives in harness-core** — this crate
//! only renders and translates keystrokes into commands.

mod app;
mod headless;
mod views;

use std::path::PathBuf;

use harness_core::agent::{LoopConfig, ResumeState, spawn_with_recorder};
use harness_core::config::{CliOverrides, Config};
use harness_core::session::{self, SessionWriter};

fn sessions_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("harness")
        .join("sessions")
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
            "--session" => args.session = Some(need_value(&mut i, "--session")?),
            "--help" | "-h" => {
                println!(
                    "harness — personal TUI coding agent (v0.1)\n\nUSAGE:\n  harness [--model M] [--base-url URL] [--project DIR]\n          [--headless \"task\" | --headless < task.txt] [--auto-approve]"
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
    let path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("harness");
    let _ = std::fs::create_dir_all(&path);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.join("harness.log"));
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

async fn load_config_and_key(args: &Args) -> anyhow::Result<(Config, Option<String>)> {
    let cfg = Config::load(&CliOverrides {
        model: args.model.clone(),
        base_url: args.base_url.clone(),
    })?;
    let key = std::env::var("HARNESS_API_KEY").ok();
    Ok((cfg, key))
}

fn main() -> anyhow::Result<()> {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("harness: {e}");
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
    let cfg = load_config_and_key(&args).await?;
    let (config, api_key) = cfg;
    let project_root = args
        .project
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // ---- session persistence (spec section 8) ------------------------
    let mut resume_state = None;
    let mut recorder = None;
    let mut session_tag: Option<String> = None;
    if args.resume || args.session.is_some() {
        let chosen = match &args.session {
            Some(id) => {
                // Accept a full path, a bare ULID, or a bare filename.
                let direct = PathBuf::from(id);
                let in_dir = sessions_dir().join(id);
                let with_ext = sessions_dir().join(format!("{id}.jsonl"));
                [direct, in_dir, with_ext].into_iter().find(|p| p.exists())
            }
            None => views::picker::pick_interactive(&sessions_dir())?,
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
        keep_recent_messages: 12,
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

    if std::env::var("HARNESS_API_KEY").is_err()
        && !config.base_url.contains("localhost")
        && !config.base_url.contains("127.0.0.1")
    {
        eprintln!(
            "warning: HARNESS_API_KEY is not set; provider calls will fail \
             (local servers like Ollama don't need one)"
        );
    }

    let (handle, ev_rx) = spawn_with_recorder(lc, resume_state, recorder);
    let mut terminal = tui_init()?;
    let res = app::run(
        &mut terminal,
        handle,
        ev_rx,
        config.clone(),
        &project_root,
        session_tag,
    )
    .await;
    tui_restore(terminal)?;
    res
}

type Tui = ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>;

fn tui_init() -> anyhow::Result<Tui> {
    use crossterm::ExecutableCommand;
    use crossterm::terminal::{EnterAlternateScreen, SetTitle, enable_raw_mode};
    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(SetTitle("harness"))?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    Ok(ratatui::Terminal::new(backend)?)
}

fn tui_restore(mut t: Tui) -> anyhow::Result<()> {
    use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
    crossterm::execute!(std::io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    t.show_cursor()?;
    Ok(())
}
