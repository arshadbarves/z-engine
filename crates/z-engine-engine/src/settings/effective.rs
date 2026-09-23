//! Effective per-session settings: the layered load, workspace trust, and
//! everything derived from settings once per (re)load (extensions,
//! instruction files, shell, environment policy, extra directories).

use std::path::{Path, PathBuf};

use z_engine_config::{
    Credentials, EnvOverrides, Extensions, Paths, SearchBackend as ConfigSearch, Settings,
    TrustStore, discover_extensions, load_with_env, resolve_search_key,
};
use z_engine_context::InstructionDoc;
use z_engine_host::{EnvPolicy, SearchBackend, ShellSpec, expand_tilde, resolve, resolve_shell};
use z_engine_protocol::NoticeLevel;

use super::instructions::instruction_docs;

/// Settings as one session applies them. Shared as `Arc` snapshots: a
/// reload swaps the whole value, so a running round sees one version.
#[derive(Debug, Clone)]
pub(crate) struct SessionSettings {
    /// Project hooks, MCP servers and checks are removed when untrusted.
    pub settings: Settings,
    pub trusted: bool,
    /// What an untrusted project defines that stays off (`hooks`,
    /// `MCP servers`, `checks`); empty when trusted.
    pub withheld: Vec<String>,
    pub extensions: Extensions,
    pub instructions: Vec<InstructionDoc>,
    pub shell: ShellSpec,
    pub env: EnvPolicy,
    pub additional_dirs: Vec<PathBuf>,
    /// The configured web search backend with its key.
    pub web_search: SearchBackend,
}

/// A load plus the notices it produced for the GUI.
#[derive(Debug)]
pub(crate) struct LoadReport {
    pub settings: SessionSettings,
    pub notices: Vec<(NoticeLevel, String)>,
}

pub(crate) fn load_session_settings(paths: &Paths, root: &Path, env: &EnvOverrides) -> LoadReport {
    let loaded = load_with_env(paths, Some(root), env);
    let mut notices: Vec<(NoticeLevel, String)> = loaded
        .warnings
        .iter()
        .map(|warning| (NoticeLevel::Warn, warning.clone()))
        .collect();
    let mut settings = loaded.settings;
    let trusted = match TrustStore::load(&paths.trust_file) {
        Ok(store) => store.is_trusted(root),
        Err(error) => {
            notices.push((
                NoticeLevel::Warn,
                format!("workspace trust unavailable: {error}"),
            ));
            false
        }
    };
    let withheld = if trusted {
        Vec::new()
    } else {
        restrict_to_user_level(paths, env, &mut settings)
    };
    if !withheld.is_empty() {
        notices.push((
            NoticeLevel::Warn,
            "This workspace is not trusted: hooks, MCP servers and checks defined by the \
             project are disabled until you trust it."
                .to_string(),
        ));
    }
    if let Some(reason) = super::sandbox::unavailable_reason(&settings) {
        notices.push((NoticeLevel::Warn, reason));
    }
    let compat = settings.compat.claude;
    let extensions = discover_extensions(paths, root, compat);
    notices.extend(extensions.errors.iter().map(|error| {
        let text = format!("skipped {}: {}", error.path, error.message);
        (NoticeLevel::Warn, text)
    }));
    let instructions = instruction_docs(paths, root, compat, &extensions);
    let shell = resolve_shell(settings.shell.path.as_deref());
    let env = EnvPolicy {
        passthrough: settings.shell.env_passthrough.clone(),
        extra: settings.shell.env.clone(),
    };
    let additional_dirs = settings
        .permissions
        .additional_directories
        .iter()
        .map(|dir| resolve(root, expand_tilde(dir, paths.home_dir.as_deref())))
        .collect();
    let web_search = web_search(paths, &settings, &mut notices);
    LoadReport {
        settings: SessionSettings {
            settings,
            trusted,
            withheld,
            extensions,
            instructions,
            shell,
            env,
            additional_dirs,
            web_search,
        },
        notices,
    }
}

/// The search backend with its key; a backend without a key is disabled
/// with a notice.
fn web_search(
    paths: &Paths,
    settings: &Settings,
    notices: &mut Vec<(NoticeLevel, String)>,
) -> SearchBackend {
    let backend = settings.web.search_backend;
    let keyed: fn(String) -> SearchBackend = match backend {
        ConfigSearch::Disabled => return SearchBackend::None,
        ConfigSearch::Searxng => {
            return match settings.web.search_url.as_deref().map(str::trim) {
                Some(url) if !url.is_empty() => SearchBackend::Searxng {
                    base_url: url.to_string(),
                },
                _ => SearchBackend::None,
            };
        }
        ConfigSearch::Brave => |api_key| SearchBackend::Brave { api_key },
        ConfigSearch::Tavily => |api_key| SearchBackend::Tavily { api_key },
        ConfigSearch::Exa => |api_key| SearchBackend::Exa { api_key },
    };
    // An unreadable key file is reported with the model client's warning.
    let key = Credentials::load(&paths.auth_file)
        .ok()
        .and_then(|credentials| resolve_search_key(&credentials, backend));
    match key {
        Some(api_key) => keyed(api_key),
        None => {
            notices.push((
                NoticeLevel::Warn,
                "web search is configured without an API key; it stays off".to_string(),
            ));
            SearchBackend::None
        }
    }
}

/// Takes hooks, MCP servers and checks from the user layer only; returns
/// which of them the project had defined differently.
fn restrict_to_user_level(
    paths: &Paths,
    env: &EnvOverrides,
    settings: &mut Settings,
) -> Vec<String> {
    let user = load_with_env(paths, None, env).settings;
    let differs = [
        ("hooks", settings.hooks != user.hooks),
        ("MCP servers", settings.mcp != user.mcp),
        (
            "checks",
            settings.verification.checks != user.verification.checks,
        ),
    ];
    settings.hooks = user.hooks;
    settings.mcp = user.mcp;
    settings.verification.checks = user.verification.checks;
    differs
        .into_iter()
        .filter(|(_, differs)| *differs)
        .map(|(what, _)| what.to_string())
        .collect()
}
