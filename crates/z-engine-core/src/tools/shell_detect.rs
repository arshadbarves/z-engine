//! Windows-only shell detection and validation (OpenCode's `shell.ts` pattern).
//!
//! Git Bash is resolved through the Git install location, never through
//! `PATH`, so WSL's non-functional `C:\Windows\System32\bash.exe` shim can
//! never be selected. Every candidate is validated before use, always with
//! a hidden window.

use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};

use super::shell::{ResolvedShell, ShellFlavor};

/// `CREATE_NO_WINDOW`: never pop a visible console for agent commands
/// (OpenCode's `windowsHide: true` equivalent).
pub(super) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Resolve an explicit config value to a validated shell.
pub(super) fn resolve_override(value: &str) -> Option<ResolvedShell> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "powershell" | "pwsh" => find_powershell().map(|path| {
            tracing::info!(?path, "using configured PowerShell shell");
            ResolvedShell {
                flavor: ShellFlavor::PowerShell,
                path,
            }
        }),
        "cmd" => {
            tracing::info!("using configured cmd.exe shell");
            Some(ResolvedShell {
                flavor: ShellFlavor::Cmd,
                path: PathBuf::from("cmd.exe"),
            })
        }
        // Bare "bash" never touches PATH: resolve through the Git install.
        "bash" | "git-bash" | "gitbash" => {
            git_bash_path().filter(|p| is_bash_usable(p)).map(|path| {
                tracing::info!(?path, "using configured Git Bash shell");
                ResolvedShell {
                    flavor: ShellFlavor::Posix,
                    path,
                }
            })
        }
        _ => {
            let path = PathBuf::from(trimmed);
            match detect_flavor_from_path(&path) {
                Some(flavor) => {
                    tracing::info!(?path, ?flavor, "using configured shell");
                    Some(ResolvedShell { flavor, path })
                }
                None => {
                    tracing::warn!(?trimmed, "configured shell_path not usable, falling back");
                    None
                }
            }
        }
    }
}

/// Detect shell flavor from an explicit absolute path.
fn detect_flavor_from_path(path: &Path) -> Option<ShellFlavor> {
    let stem = path.file_stem()?.to_str()?.to_ascii_lowercase();
    match stem.as_str() {
        "bash" => {
            if is_wsl_shim(path) {
                tracing::warn!(?path, "refusing WSL bash.exe shim");
                return None;
            }
            if path.is_file() && is_bash_usable(path) {
                Some(ShellFlavor::Posix)
            } else {
                None
            }
        }
        "pwsh" | "powershell" => {
            if is_powershell_usable(path) {
                Some(ShellFlavor::PowerShell)
            } else {
                None
            }
        }
        "cmd" => Some(ShellFlavor::Cmd),
        _ => None,
    }
}

/// `C:\Windows\System32\bash.exe` is a WSL stub that fails when WSL is not
/// enabled. It must never be selected as the agent shell.
fn is_wsl_shim(path: &Path) -> bool {
    let is_bash = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("bash"))
        .unwrap_or(false);
    if !is_bash {
        return false;
    }
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("system32"))
        .unwrap_or(false)
}

/// Locate Git Bash through the Git install location (OpenCode's `gitbash()`),
/// never through `PATH`, so WSL's shim cannot leak in.
fn git_bash_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ZENGINE_GIT_BASH_PATH") {
        let pb = PathBuf::from(p.trim());
        if pb.is_file() {
            return Some(pb);
        }
    }
    let git = find_in_path("git")?;
    let root = git.parent()?.parent()?;
    let cand = root.join("bin").join("bash.exe");
    if cand.is_file() {
        return Some(cand);
    }
    let alt = root.join("usr").join("bin").join("bash.exe");
    if alt.is_file() {
        return Some(alt);
    }
    None
}

/// Find a working PowerShell executable (`pwsh.exe` first, then
/// `powershell.exe`), including well-known install locations.
pub(super) fn find_powershell() -> Option<PathBuf> {
    if let Some(path) = find_in_path("pwsh") {
        if is_powershell_usable(&path) {
            return Some(path);
        }
    }
    if let Some(path) = find_in_path("powershell") {
        if is_powershell_usable(&path) {
            return Some(path);
        }
    }
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

/// Validate that PowerShell actually works (hidden window).
fn is_powershell_usable(path: &Path) -> bool {
    test_shell(path, &["-NoProfile", "-Command", "Write-Output ok"])
}

/// Validate that a Git Bash executable actually works. WSL's shim fails
/// `--version` when WSL is not enabled, so this doubles as a WSL guard.
fn is_bash_usable(path: &Path) -> bool {
    test_shell(path, &["--version"])
}

/// Test if a shell executable works by running a trivial command.
/// Always hidden: detection must never flash a console window.
fn test_shell(program: &Path, args: &[&str]) -> bool {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    cmd.status().map(|s| s.success()).unwrap_or(false)
}
