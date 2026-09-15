use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::VerificationError;

pub(super) fn identity(root: &Path) -> Result<String, VerificationError> {
    let rustc = version(root, "rustc", &["--version", "--verbose"])?;
    let cargo = version(root, "cargo", &["--version"])?;
    let mut environment = Sha256::new();
    for (key, value) in super::environment::effective() {
        environment.update(key.as_encoded_bytes());
        environment.update([0]);
        environment.update(value.as_encoded_bytes());
        environment.update([0]);
    }
    // Cargo inherits global/ancestor config; bind it into validity rather than omit it.
    let mut configurations = std::collections::BTreeSet::new();
    for ancestor in root.ancestors() {
        configurations.insert(ancestor.join(".cargo"));
    }
    if let Some(home) = std::env::var_os("CARGO_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".cargo")))
    {
        configurations.insert(home);
    }
    for directory in configurations {
        for name in ["config", "config.toml"] {
            let path = directory.join(name);
            if path.exists() {
                let (digest, _) = super::artifacts::digest_file(&path, 1024 * 1024)?;
                super::cargo_scope::validate_config(root, &path)?;
                environment.update(path.as_os_str().as_encoded_bytes());
                environment.update(digest.as_bytes());
            }
        }
    }
    Ok(format!(
        "{}\n{}\nenvironment-sha256:{:x}",
        rustc.trim(),
        cargo.trim(),
        environment.finalize()
    ))
}

fn version(root: &Path, binary: &str, args: &[&str]) -> Result<String, VerificationError> {
    let mut command = Command::new(binary);
    super::environment::apply(&mut command);
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| VerificationError::Process(format!("cannot identify {binary}: {e}")))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                super::process::kill_group(child.id());
                if let Err(cleanup) = child.kill().and_then(|()| child.wait().map(|_| ())) {
                    tracing::warn!(%cleanup, "toolchain probe reap failed after wait error");
                }
                return Err(VerificationError::Process(format!(
                    "cannot wait for {binary}: {error}"
                )));
            }
        }
        if started.elapsed() > Duration::from_secs(10) {
            super::process::kill_group(child.id());
            if let Err(error) = child.kill() {
                tracing::warn!(%error, "toolchain probe termination failed");
            }
            child
                .wait()
                .map_err(|e| VerificationError::Process(e.to_string()))?;
            return Err(VerificationError::Process(format!(
                "{binary} identity timed out"
            )));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if !status.success() {
        return Err(VerificationError::Process(format!(
            "{binary} identity exited {status}"
        )));
    }
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(|| VerificationError::Process("toolchain probe output unavailable".into()))?
        .take(32769)
        .read_to_end(&mut bytes)
        .map_err(|e| VerificationError::Process(e.to_string()))?;
    if bytes.len() > 32768 {
        return Err(VerificationError::ScanLimit(
            "toolchain identity output".into(),
        ));
    }
    String::from_utf8(bytes).map_err(|e| VerificationError::Process(e.to_string()))
}
