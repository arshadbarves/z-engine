use std::collections::BTreeMap;
use std::ffi::OsString;

const ALLOWED: &[&str] = &[
    "PATH",
    "HOME",
    "SHELL",
    "TERM",
    "LANG",
    "LC_ALL",
    "TMPDIR",
    "TMP",
    "TEMP",
    "USER",
    "LOGNAME",
    "SystemRoot",
    "SYSTEMROOT",
    "WINDIR",
    "windir",
    "COMSPEC",
    "ComSpec",
    "PATHEXT",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "USERPROFILE",
    "ALLUSERSPROFILE",
    "PROGRAMDATA",
    "HOMEDRIVE",
    "HOMEPATH",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "RUSTFLAGS",
    "RUSTDOCFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_ENCODED_RUSTDOCFLAGS",
    "RUSTC",
    "RUSTDOC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_TARGET",
    "CARGO_BUILD_JOBS",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_BUILD_RUSTDOCFLAGS",
    "CARGO_TARGET_DIR",
    "CARGO_INCREMENTAL",
    "CARGO_NET_OFFLINE",
    "CARGO_TERM_COLOR",
    "CARGO_TERM_VERBOSE",
    "CARGO_TERM_QUIET",
    "CC",
    "CXX",
    "AR",
    "CFLAGS",
    "CXXFLAGS",
    "LDFLAGS",
    "PKG_CONFIG_PATH",
    "SDKROOT",
    "MACOSX_DEPLOYMENT_TARGET",
];

pub(super) fn effective() -> BTreeMap<OsString, OsString> {
    filter(std::env::vars_os())
}

fn filter(values: impl IntoIterator<Item = (OsString, OsString)>) -> BTreeMap<OsString, OsString> {
    let mut environment: BTreeMap<_, _> = values
        .into_iter()
        .filter(|(key, _)| key.to_str().is_some_and(|key| ALLOWED.contains(&key)))
        .collect();
    environment.insert("RUSTUP_AUTO_INSTALL".into(), "0".into());
    environment
}

pub(super) fn apply(command: &mut std::process::Command) {
    command.env_clear().envs(effective());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_environment_excludes_provider_and_registry_credentials() {
        let environment = filter(
            [
                ("PATH", "/usr/bin"),
                ("RUSTFLAGS", "-C opt-level=1"),
                ("CARGO_HOME", "/home/example/.cargo"),
                ("OPENAI_API_KEY", "secret"),
                ("OPENROUTER_API_KEY", "secret"),
                ("ANTHROPIC_API_KEY", "secret"),
                ("CARGO_REGISTRY_TOKEN", "secret"),
                ("CARGO_REGISTRIES_PRIVATE_TOKEN", "secret"),
                ("AWS_SECRET_ACCESS_KEY", "secret"),
                ("RUSTUP_AUTO_INSTALL", "1"),
            ]
            .map(|(key, value)| (key.into(), value.into())),
        );
        assert_eq!(environment.len(), 4);
        assert_eq!(
            environment.get(&OsString::from("PATH")).unwrap(),
            "/usr/bin"
        );
        assert_eq!(
            environment
                .get(&OsString::from("RUSTUP_AUTO_INSTALL"))
                .unwrap(),
            "0"
        );
    }
}
