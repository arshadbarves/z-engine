//! Pick `sh`/`bash`, PowerShell, or `cmd.exe` for the bash tool and hooks.
//!
//! Windows shell detection follows a professional fallback chain:
//! 1. Config override (`shell_path` in config.toml)
//! 2. PowerShell (pwsh.exe → powershell.exe) — always available
//! 3. cmd.exe (ultimate fallback)
//!
//! Each candidate is validated before use to prevent WSL's non-functional
//! `bash.exe` from being selected.

#[cfg(windows)]
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShellFlavor {
    Posix,
    #[cfg_attr(not(windows), allow(dead_code))]
    PowerShell,
    #[cfg_attr(not(windows), allow(dead_code))]
    Cmd,
}

/// Cached shell detection result (computed once at startup).
static SHELL_CACHE: std::sync::OnceLock<ResolvedShell> = std::sync::OnceLock::new();

#[derive(Debug, Clone)]
struct ResolvedShell {
    flavor: ShellFlavor,
    /// Absolute path to the shell executable (Windows only).
    #[cfg(windows)]
    path: PathBuf,
}

/// Initialize the shell resolver with an optional config override.
/// Must be called once at startup before any shell operations.
pub(super) fn init(config_shell_path: Option<&str>) {
    let _ = SHELL_CACHE.get_or_init(|| resolve(config_shell_path));
}

fn resolve(config_shell_path: Option<&str>) -> ResolvedShell {
    #[cfg(windows)]
    {
        // 1. Config override takes priority
        if let Some(path_str) = config_shell_path {
            let path = PathBuf::from(path_str);
            if let Some(flavor) = detect_flavor_from_path(&path) {
                tracing::info!(?path, ?flavor, "using configured shell");
                return ResolvedShell { flavor, path };
            }
            tracing::warn!(?path_str, "configured shell_path not usable, falling back");
        }

        // 2. Try PowerShell (always available on Windows)
        if let Some(path) = find_powershell() {
            tracing::info!(?path, "using PowerShell as default shell");
            return ResolvedShell {
                flavor: ShellFlavor::PowerShell,
                path,
            };
        }

        // 3. Ultimate fallback: cmd.exe
        tracing::info!("using cmd.exe as fallback shell");
        ResolvedShell {
            flavor: ShellFlavor::Cmd,
            path: PathBuf::from("cmd.exe"),
        }
    }
    #[cfg(not(windows))]
    {
        // On Unix, config_shell_path is not used; always use sh
        let _ = config_shell_path;
        ResolvedShell {
            flavor: ShellFlavor::Posix,
        }
    }
}

/// Detect shell flavor from an explicit path.
#[cfg(windows)]
fn detect_flavor_from_path(path: &Path) -> Option<ShellFlavor> {
    let name = path.file_stem()?.to_str()?.to_ascii_lowercase();
    match name.as_str() {
        "bash" | "bash.exe" => {
            if is_bash_usable(path) {
                Some(ShellFlavor::Posix)
            } else {
                None
            }
        }
        "pwsh" | "pwsh.exe" | "powershell" | "powershell.exe" => Some(ShellFlavor::PowerShell),
        "cmd" | "cmd.exe" => Some(ShellFlavor::Cmd),
        _ => None,
    }
}

/// Find a working PowerShell executable (pwsh.exe first, then powershell.exe).
#[cfg(windows)]
fn find_powershell() -> Option<PathBuf> {
    // Try pwsh (PowerShell 7+) first
    if let Some(path) = find_in_path("pwsh") {
        if is_powershell_usable(&path) {
            return Some(path);
        }
    }
    // Try powershell (Windows PowerShell 5.1)
    if let Some(path) = find_in_path("powershell") {
        if is_powershell_usable(&path) {
            return Some(path);
        }
    }
    // Check common installation paths
    let fallbacks: &[&str] = &[
        r#"C:\Program Files\PowerShell\7\pwsh.exe"#,
        r#"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"#,
    ];
    for fallback in fallbacks {
        let path = PathBuf::from(fallback);
        if path.exists() && is_powershell_usable(&path) {
            return Some(path);
        }
    }
    None
}

/// Search PATH for an executable.
#[cfg(windows)]
fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let exts: &[&str] = &["", ".exe", ".cmd", ".bat"];
    for dir in std::env::split_paths(&path_var) {
        for ext in exts {
            let cand = dir.join(format!("{name}{ext}"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// Validate that PowerShell actually works.
#[cfg(windows)]
fn is_powershell_usable(path: &Path) -> bool {
    test_shell(path, &["-NoProfile", "-Command", "Write-Output ok"])
}

/// Validate that bash actually works (prevents WSL's non-functional bash.exe).
#[cfg(windows)]
fn is_bash_usable(path: &Path) -> bool {
    test_shell(path, &["--version"])
}

/// Test if a shell executable works by running a simple command.
#[cfg(windows)]
fn test_shell(program: &Path, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub(super) fn flavor() -> ShellFlavor {
    SHELL_CACHE
        .get()
        .map(|s| s.flavor)
        .unwrap_or(ShellFlavor::Posix)
}

pub(super) fn program() -> &'static str {
    if cfg!(windows) {
        match flavor() {
            ShellFlavor::Posix => "bash",
            ShellFlavor::PowerShell => "powershell",
            ShellFlavor::Cmd => "cmd",
        }
    } else {
        "sh"
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
