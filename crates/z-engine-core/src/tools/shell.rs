//! Pick `sh`/`bash` or Windows `cmd.exe` for the bash tool and hooks.

#[cfg(windows)]
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShellFlavor {
    Posix,
    #[cfg_attr(not(windows), allow(dead_code))]
    Cmd,
}

pub(super) fn flavor() -> ShellFlavor {
    #[cfg(windows)]
    {
        if lookup("bash").is_some() {
            ShellFlavor::Posix
        } else {
            ShellFlavor::Cmd
        }
    }
    #[cfg(not(windows))]
    ShellFlavor::Posix
}

pub(super) fn program() -> &'static str {
    if cfg!(windows) {
        match flavor() {
            ShellFlavor::Posix => "bash",
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
            ShellFlavor::Cmd => "/C",
        }
    } else {
        "-c"
    }
}

#[cfg(windows)]
fn lookup(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: &[&str] = &["", ".exe", ".cmd", ".bat"];
    for dir in std::env::split_paths(&path) {
        for ext in exts {
            let cand = dir.join(format!("{name}{ext}"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
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
}
