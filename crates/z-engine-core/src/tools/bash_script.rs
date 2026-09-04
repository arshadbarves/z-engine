//! Persistent-cwd script assembly for the `bash` tool: wraps the model's
//! command in a probe that reports the shell's final working directory on
//! stderr, plus parsing of that marker back out.

use std::path::{Path, PathBuf};

use super::shell::{ShellFlavor, cmd_quote, flavor};

/// Marker byte (SOH) delimiting the embedded cwd probe on stderr.
const MARKER: char = '\u{01}';
const MARKER_TAG: &str = "ZENGINE_CWD:";

pub(super) fn build_script(start_cwd: &Path, command: &str) -> String {
    match flavor() {
        ShellFlavor::Cmd => build_cmd_script(start_cwd, command),
        ShellFlavor::PowerShell => build_powershell_script(start_cwd, command),
        ShellFlavor::Posix => format!(
            "cd {} || true\n{}\nstatus=$?\nprintf '\\{}{}%s\\{}' \"$PWD\" >&2\nexit $status\n",
            shell_quote(&start_cwd.to_string_lossy()),
            command,
            MARKER as u32,
            MARKER_TAG,
            MARKER as u32
        ),
    }
}

fn build_cmd_script(start_cwd: &Path, command: &str) -> String {
    format!(
        "cd /d {} 2>nul\r\n{}\r\nset ZENGINE_STATUS=%ERRORLEVEL%\r\necho {MARKER}{MARKER_TAG}%CD%{MARKER} 1>&2\r\nexit /b %ZENGINE_STATUS%\r\n",
        cmd_quote(&start_cwd.to_string_lossy()),
        command,
    )
}

fn build_powershell_script(start_cwd: &Path, command: &str) -> String {
    // PowerShell script: set cwd, run the command, capture the exit code,
    // and report the *live* working directory on stderr via a clean
    // `[Console]::Error` write (Write-Error would add noisy decorations).
    format!(
        "Set-Location -LiteralPath '{start_cwd}' -ErrorAction SilentlyContinue\n\
         {command}\n\
         $ZENGINE_STATUS = $LASTEXITCODE\n\
         if ($null -eq $ZENGINE_STATUS) {{ $ZENGINE_STATUS = 0 }}\n\
         [Console]::Error.WriteLine('{marker}{marker_tag}' + (Get-Location).Path + '{marker}')\n\
         exit $ZENGINE_STATUS",
        start_cwd = ps_quote(&start_cwd.to_string_lossy()),
        command = command,
        marker = MARKER,
        marker_tag = MARKER_TAG,
    )
}

/// PowerShell single-quoted string escaping: double any single quotes.
fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Remove the trailing cwd marker from `stderr`, returning the recorded dir.
pub(super) fn extract_marker(stderr: &mut String) -> Option<PathBuf> {
    let needle = format!("{MARKER}{MARKER_TAG}");
    let start = stderr.find(&needle)?;
    let rest = &stderr[start + needle.len()..];
    let end = rest.find(MARKER)?;
    let dir_text = rest[..end].to_string();
    // Cut everything from the marker onward, trimming the newline before it.
    *stderr = stderr[..start].trim_end_matches(['\n', '\r']).to_string();
    if dir_text.is_empty() {
        None
    } else {
        Some(PathBuf::from(dir_text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_extraction_roundtrip() {
        let mut stderr = format!("some noise\n{MARKER}{MARKER_TAG}/tmp/x/y{MARKER}\n");
        let dir = extract_marker(&mut stderr);
        assert_eq!(dir, Some(std::path::PathBuf::from("/tmp/x/y")));
        assert_eq!(stderr, "some noise");

        let mut plain = "just errors".to_string();
        assert_eq!(extract_marker(&mut plain), None);
        assert_eq!(plain, "just errors");
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("/tmp/a b"), "'/tmp/a b'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn powershell_script_reports_live_cwd_and_defaults_exit_zero() {
        let script = build_powershell_script(std::path::Path::new(r"C:\proj"), "cargo test");
        assert!(script.contains("Set-Location -LiteralPath 'C:\\proj'"));
        assert!(script.contains("(Get-Location).Path"));
        assert!(script.contains("$ZENGINE_STATUS = 0"));
        assert!(script.contains("Console]::Error.WriteLine"));
    }

    #[test]
    fn ps_quote_doubles_single_quotes() {
        assert_eq!(ps_quote("it's"), "it''s");
    }
}
