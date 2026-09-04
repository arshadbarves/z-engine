//! Shell selection for the bash tool and hooks.
//!
//! Resolution order on Windows (OpenCode's pattern):
//! 1. Config override (`shell_path` / `ZENGINE_SHELL`)
//! 2. PowerShell (`pwsh.exe` → `powershell.exe`)
//! 3. `cmd.exe`
//!
//! Windows detection/validation lives in [`super::shell_detect`]; this file
//! only holds the cached result and the spawn parameters.

use std::path::PathBuf;

#[cfg(windows)]
use super::shell_detect::{find_powershell, resolve_override};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShellFlavor {
    Posix,
    #[cfg_attr(not(windows), allow(dead_code))]
    PowerShell,
    #[cfg_attr(not(windows), allow(dead_code))]
    Cmd,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedShell {
    pub(super) flavor: ShellFlavor,
    /// Absolute path to the shell executable (Windows only).
    #[cfg(windows)]
    pub(super) path: PathBuf,
}

/// Cached shell detection result (computed once at startup).
static SHELL_CACHE: std::sync::OnceLock<ResolvedShell> = std::sync::OnceLock::new();

/// Initialize the shell resolver with an optional config override.
/// Must be called once at startup before any shell operations.
pub(super) fn init(config_shell_path: Option<&str>) {
    let _ = SHELL_CACHE.get_or_init(|| resolve(config_shell_path));
}

/// Lazily resolve with no override when `init` was never called, so
/// `flavor()` can never return a wrong default on Windows.
#[cfg(windows)]
fn ensure_init() {
    if SHELL_CACHE.get().is_none() {
        init(None);
    }
}

fn resolve(config_shell_path: Option<&str>) -> ResolvedShell {
    #[cfg(windows)]
    {
        // 1. Config override takes priority.
        if let Some(r) = config_shell_path.and_then(resolve_override) {
            return r;
        }

        // 2. PowerShell is always available on Windows.
        if let Some(path) = find_powershell() {
            tracing::info!(?path, "using PowerShell as default shell");
            return ResolvedShell {
                flavor: ShellFlavor::PowerShell,
                path,
            };
        }

        // 3. Ultimate fallback: cmd.exe.
        tracing::info!("using cmd.exe as fallback shell");
        ResolvedShell {
            flavor: ShellFlavor::Cmd,
            path: PathBuf::from("cmd.exe"),
        }
    }
    #[cfg(not(windows))]
    {
        // On Unix, config_shell_path is not used; always use sh.
        let _ = config_shell_path;
        ResolvedShell {
            flavor: ShellFlavor::Posix,
        }
    }
}

pub(super) fn flavor() -> ShellFlavor {
    #[cfg(windows)]
    ensure_init();
    SHELL_CACHE
        .get()
        .map(|s| s.flavor)
        .unwrap_or(ShellFlavor::Posix)
}

/// Absolute path of the resolved shell executable.
pub(super) fn program_path() -> PathBuf {
    #[cfg(windows)]
    {
        ensure_init();
        SHELL_CACHE
            .get()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| PathBuf::from("cmd.exe"))
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("sh")
    }
}

pub(super) fn flag() -> &'static str {
    if cfg!(windows) {
        match flavor() {
            ShellFlavor::Posix => "-lc",
            ShellFlavor::PowerShell => "-NoProfile",
            ShellFlavor::Cmd => "/C",
        }
    } else {
        "-c"
    }
}

/// For PowerShell, the `-Command` flag follows `-NoProfile`.
pub(super) fn powershell_command_flag() -> &'static str {
    "-Command"
}

/// Whether the resolved shell is PowerShell (affects script building).
pub(super) fn is_powershell() -> bool {
    cfg!(windows) && flavor() == ShellFlavor::PowerShell
}

#[cfg(windows)]
pub(super) use super::shell_detect::CREATE_NO_WINDOW;

#[cfg(windows)]
const WINDOWS_ENV: &[&str] = &[
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "USERNAME",
    "TEMP",
    "TMP",
    "PATHEXT",
    "COMSPEC",
    "SystemRoot",
    "windir",
    "APPDATA",
    "LOCALAPPDATA",
    "PSModulePath",
    "Path",
];

pub(super) fn extra_env_keys() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        WINDOWS_ENV
    }
    #[cfg(not(windows))]
    {
        &[]
    }
}

/// `cmd.exe` quoting: wrap in double quotes, double any inner quotes.
pub(super) fn cmd_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_quote_wraps_and_doubles_quotes() {
        assert_eq!(cmd_quote(r"C:\a b"), r#""C:\a b""#);
        assert_eq!(cmd_quote(r#"say "hi""#), r#""say ""hi""""#);
    }

    #[test]
    fn powershell_command_flag_is_dash_command() {
        assert_eq!(powershell_command_flag(), "-Command");
    }
}
