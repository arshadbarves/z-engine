//! What a work order may ask the harness to execute unattended.
//!
//! Acceptance commands are the one place a model's own words become a
//! process this harness runs *without an approval prompt*, so the policy
//! is a closed list rather than a filter: the program must be `cargo`,
//! the subcommand must be one of a handful that only build, check, or
//! test the project in front of it, and every argument must be unable to
//! redirect that work somewhere else.
//!
//! Rejected on purpose, each because it turns "prove your change" into
//! "run this for me": external subcommands (`cargo <anything>` resolves
//! to a `cargo-<anything>` binary on `PATH`), `install`/`run`, anything
//! that fetches from a registry or a git URL, and any argument naming an
//! alternative manifest, config, output directory, or toolchain — cargo's
//! `--config` alone can install a target runner, which is arbitrary code
//! execution by another name.
//!
//! The policy is enforced twice: at work-order admission, so the model
//! learns immediately and in its own vocabulary, and again in
//! [`super::command_run`] before a process is spawned, so an order that
//! reached the runner by any other route still cannot execute.

/// Cargo subcommands verification will run. Narrow on purpose: each one
/// only compiles, lints, formats, or tests the project it is pointed at.
pub const SAFE_CARGO_SUBCOMMANDS: &[&str] = &["build", "check", "clippy", "fmt", "test"];

/// Arguments that would let a permitted subcommand act on something
/// other than this project, or execute something the project did not
/// declare. Matched on the flag name, so `--config=x` and `--config x`
/// are both refused.
const DENIED_FLAGS: &[(&str, &str)] = &[
    (
        "--config",
        "cargo config can install a target runner, which executes arbitrary programs",
    ),
    (
        "--config-path",
        "an alternative config file can redirect what runs",
    ),
    (
        "--manifest-path",
        "verification must compile this project, not a manifest chosen by the order",
    ),
    (
        "--target-dir",
        "build output must stay where this project puts it",
    ),
    (
        "--out-dir",
        "build output must stay where this project puts it",
    ),
    (
        "--artifact-dir",
        "build output must stay where this project puts it",
    ),
    ("--index", "verification never talks to a package registry"),
    (
        "--registry",
        "verification never talks to a package registry",
    ),
    ("--git", "verification never fetches from a git source"),
    ("--path", "verification never installs from a path source"),
    (
        "-Z",
        "unstable cargo flags are outside what this harness will run unattended",
    ),
    (
        "--unstable-flags",
        "unstable cargo flags are outside what this harness will run unattended",
    ),
];

/// Which commands a runner will execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandPolicy {
    /// Production policy: `cargo`, one of [`SAFE_CARGO_SUBCOMMANDS`], and
    /// arguments that cannot redirect the work.
    Cargo,
    /// A bare program allowlist with no argument policy. Test-only: it
    /// exists so runner tests can reach the timeout, missing-program, and
    /// rejection paths without pretending cargo misbehaves.
    Programs(Vec<String>),
}

impl CommandPolicy {
    pub fn programs(names: &[&str]) -> Self {
        Self::Programs(names.iter().map(|n| (*n).to_string()).collect())
    }

    fn allowed_label(&self) -> String {
        match self {
            Self::Cargo => format!("cargo {}", SAFE_CARGO_SUBCOMMANDS.join("|")),
            Self::Programs(names) => names.join(", "),
        }
    }
}

/// Why an acceptance command will not be run. Messages reach the model
/// verbatim, so each one says what would be acceptable instead.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AcceptanceError {
    #[error("an acceptance command must name a program")]
    Empty,
    #[error(
        "`{command}` contains the shell character `{ch}`; acceptance commands are run directly, \
         without a shell"
    )]
    ShellSyntax { command: String, ch: char },
    #[error("`{program}` is not a program this harness will run unattended (allowed: {allowed})")]
    ProgramNotAllowed { program: String, allowed: String },
    #[error(
        "`{argument}` selects a toolchain; acceptance commands run with the toolchain this \
         project already uses"
    )]
    ToolchainOverride { argument: String },
    #[error("`cargo` needs a subcommand; allowed: {allowed}")]
    MissingSubcommand { allowed: String },
    #[error(
        "`cargo {subcommand}` is not a verification subcommand this harness will run \
         (allowed: {allowed})"
    )]
    SubcommandNotAllowed { subcommand: String, allowed: String },
    #[error("`{argument}` is not allowed in an acceptance command: {reason}")]
    ArgumentNotAllowed { argument: String, reason: String },
}

/// Split `command` into argv and check it against `policy`.
///
/// Returns the argv a runner may spawn, so a caller cannot accidentally
/// validate one string and execute another.
pub fn validate(command: &str, policy: &CommandPolicy) -> Result<Vec<String>, AcceptanceError> {
    let args = argv(command)?;
    match policy {
        CommandPolicy::Programs(names) => {
            if !names.iter().any(|p| *p == args[0]) {
                return Err(AcceptanceError::ProgramNotAllowed {
                    program: args[0].clone(),
                    allowed: policy.allowed_label(),
                });
            }
        }
        CommandPolicy::Cargo => check_cargo(&args, policy)?,
    }
    Ok(args)
}

/// Split `command` into argv, refusing anything a shell would have to
/// interpret. Verification never needs a pipeline; accepting one would
/// mean accepting an unbounded, unallowlisted write set.
fn argv(command: &str) -> Result<Vec<String>, AcceptanceError> {
    const SHELL_CHARS: &[char] = &[
        '|', '&', ';', '<', '>', '(', ')', '$', '`', '\\', '"', '\'', '\n', '*', '?', '~', '{', '}',
    ];
    let mut out = Vec::new();
    for token in command.split_whitespace() {
        if let Some(ch) = token.chars().find(|c| SHELL_CHARS.contains(c)) {
            return Err(AcceptanceError::ShellSyntax {
                command: command.to_string(),
                ch,
            });
        }
        out.push(token.to_string());
    }
    if out.is_empty() {
        return Err(AcceptanceError::Empty);
    }
    Ok(out)
}

fn check_cargo(args: &[String], policy: &CommandPolicy) -> Result<(), AcceptanceError> {
    let allowed = policy.allowed_label();
    if args[0] != "cargo" {
        return Err(AcceptanceError::ProgramNotAllowed {
            program: args[0].clone(),
            allowed,
        });
    }
    let Some(subcommand) = args.get(1) else {
        return Err(AcceptanceError::MissingSubcommand { allowed });
    };
    if subcommand.starts_with('+') {
        return Err(AcceptanceError::ToolchainOverride {
            argument: subcommand.clone(),
        });
    }
    if !SAFE_CARGO_SUBCOMMANDS.contains(&subcommand.as_str()) {
        return Err(AcceptanceError::SubcommandNotAllowed {
            subcommand: subcommand.clone(),
            allowed,
        });
    }
    for argument in &args[2..] {
        check_argument(argument)?;
    }
    Ok(())
}

/// One argument of an already-permitted subcommand. Everything here is
/// about *where* the work happens: an argument may narrow what cargo
/// builds or tests, but it may not point cargo at another project, another
/// config, or another toolchain.
fn check_argument(argument: &str) -> Result<(), AcceptanceError> {
    if argument == "--" {
        return Ok(()); // the separator itself; what follows is checked too
    }
    if argument.starts_with('+') {
        return Err(AcceptanceError::ToolchainOverride {
            argument: argument.to_string(),
        });
    }
    let name = argument.split('=').next().unwrap_or(argument);
    if let Some((_, reason)) = DENIED_FLAGS
        .iter()
        .find(|(flag, _)| *flag == name || (flag.len() == 2 && argument.starts_with(flag)))
    {
        return Err(AcceptanceError::ArgumentNotAllowed {
            argument: argument.to_string(),
            reason: (*reason).to_string(),
        });
    }
    let value = argument.split_once('=').map_or(argument, |(_, v)| v);
    if is_escaping_path(value) {
        return Err(AcceptanceError::ArgumentNotAllowed {
            argument: argument.to_string(),
            reason: "an acceptance command may only act on paths inside this project".into(),
        });
    }
    Ok(())
}

/// Whether `value` names a location outside the project: an absolute
/// path, a Windows drive-qualified path, or a path that walks upward.
fn is_escaping_path(value: &str) -> bool {
    let looks_absolute = value.starts_with('/')
        || value.starts_with('\\')
        || value
            .as_bytes()
            .get(1)
            .is_some_and(|b| *b == b':' && value.as_bytes()[0].is_ascii_alphabetic());
    looks_absolute || value.split(['/', '\\']).any(|component| component == "..")
}

#[cfg(test)]
mod tests;
