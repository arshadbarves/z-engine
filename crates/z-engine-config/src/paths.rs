//! On-disk locations. The user config directory is shared with v1
//! (`~/.config/z-engine`, `%APPDATA%\z-engine` on Windows) and data lives in
//! the platform data directory, so v1 files are found where v1 left them.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::ConfigError;
use crate::files::write_atomic;
use crate::migrate::{MigrationOutcome, migrate_file};

pub const APP_DIR: &str = "z-engine";
pub const PROJECT_DIR: &str = ".z-engine";
pub const CONFIG_DIR_ENV: &str = "ZENGINE_CONFIG_DIR";
pub const DATA_DIR_ENV: &str = "ZENGINE_DATA_DIR";

const DEFAULT_USER_CONFIG: &str = include_str!("default_config.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    /// v1 `<ulid>.jsonl` session files and v2 per-session directories.
    pub sessions_dir: PathBuf,
    pub checkpoints_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub user_config_file: PathBuf,
    pub auth_file: PathBuf,
    pub trust_file: PathBuf,
    pub models_file: PathBuf,
    /// Bounds the instruction-file walk and locates `~/.claude`; `None`
    /// disables both (the [`Paths::with_roots`] default).
    pub home_dir: Option<PathBuf>,
}

impl Paths {
    /// `$ZENGINE_CONFIG_DIR` / `$ZENGINE_DATA_DIR`, else the v1 locations.
    pub fn discover() -> Result<Self, ConfigError> {
        Self::discover_from(
            |name| std::env::var_os(name),
            dirs::home_dir(),
            dirs::data_dir(),
        )
    }

    fn discover_from(
        var: impl Fn(&str) -> Option<OsString>,
        home: Option<PathBuf>,
        platform_data: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        let from_env = |name: &str| {
            var(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        let config_dir = from_env(CONFIG_DIR_ENV)
            .or_else(|| default_config_dir(home.as_deref()))
            .ok_or(ConfigError::NoDirectory {
                kind: "config",
                env_var: CONFIG_DIR_ENV,
            })?;
        let data_dir = from_env(DATA_DIR_ENV)
            .or_else(|| platform_data.map(|dir| dir.join(APP_DIR)))
            .ok_or(ConfigError::NoDirectory {
                kind: "data",
                env_var: DATA_DIR_ENV,
            })?;
        Ok(Self {
            home_dir: home,
            ..Self::with_roots(config_dir, data_dir)
        })
    }

    /// Every path under explicit roots, with no home directory.
    pub fn with_roots(config_dir: impl Into<PathBuf>, data_dir: impl Into<PathBuf>) -> Self {
        let config_dir = config_dir.into();
        let data_dir = data_dir.into();
        Self {
            sessions_dir: data_dir.join("sessions"),
            checkpoints_dir: data_dir.join("checkpoints"),
            cache_dir: data_dir.join("cache"),
            user_config_file: config_dir.join("config.toml"),
            auth_file: config_dir.join("auth.json"),
            trust_file: config_dir.join("trust.json"),
            models_file: config_dir.join("models.json"),
            config_dir,
            data_dir,
            home_dir: None,
        }
    }

    /// `~/.claude`, read for Claude Code compatibility.
    pub fn claude_user_dir(&self) -> Option<PathBuf> {
        self.home_dir.as_ref().map(|home| home.join(".claude"))
    }

    /// Creates the directories and a commented default user config. An
    /// existing v1 user config is migrated in place with a backup; a failed
    /// migration is reported in the outcome's notes, not as an error, and
    /// [`crate::load`] then reads the file as v1 and reports it again.
    pub fn ensure(&self) -> Result<MigrationOutcome, ConfigError> {
        let dirs = [
            &self.config_dir,
            &self.data_dir,
            &self.sessions_dir,
            &self.checkpoints_dir,
            &self.cache_dir,
        ];
        for dir in dirs {
            fs::create_dir_all(dir).map_err(|error| ConfigError::io(dir, error))?;
        }
        let file = &self.user_config_file;
        match fs::symlink_metadata(file) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                write_atomic(file, DEFAULT_USER_CONFIG.as_bytes(), false)?;
                return Ok(MigrationOutcome::default());
            }
            Err(error) => return Err(ConfigError::io(file, error)),
            Ok(_) => {}
        }
        Ok(migrate_file(file).unwrap_or_else(|error| {
            tracing::warn!(path = %file.display(), %error, "user settings were not migrated");
            MigrationOutcome {
                notes: vec![format!("not migrated: {error}")],
                ..MigrationOutcome::default()
            }
        }))
    }
}

/// `<root>/.z-engine`.
pub fn project_dir(root: &Path) -> PathBuf {
    root.join(PROJECT_DIR)
}

/// `<root>/.z-engine/config.toml`, shared with the team.
pub fn project_config_file(root: &Path) -> PathBuf {
    project_dir(root).join("config.toml")
}

/// `<root>/.z-engine/config.local.toml`, personal and not committed.
pub fn project_local_file(root: &Path) -> PathBuf {
    project_dir(root).join("config.local.toml")
}

#[cfg(windows)]
fn default_config_dir(home: Option<&Path>) -> Option<PathBuf> {
    dirs::config_dir()
        .or_else(|| home.map(Path::to_path_buf))
        .map(|dir| dir.join(APP_DIR))
}

#[cfg(not(windows))]
fn default_config_dir(home: Option<&Path>) -> Option<PathBuf> {
    home.map(|home| home.join(".config").join(APP_DIR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EnvOverrides, Settings, load_with_env};

    #[test]
    fn roots_lay_out_every_file() {
        let paths = Paths::with_roots("/c", "/d");
        assert_eq!(paths.user_config_file, Path::new("/c/config.toml"));
        assert_eq!(paths.auth_file, Path::new("/c/auth.json"));
        assert_eq!(paths.trust_file, Path::new("/c/trust.json"));
        assert_eq!(paths.models_file, Path::new("/c/models.json"));
        assert_eq!(paths.sessions_dir, Path::new("/d/sessions"));
        assert_eq!(paths.checkpoints_dir, Path::new("/d/checkpoints"));
        assert_eq!(paths.cache_dir, Path::new("/d/cache"));
        assert!(paths.claude_user_dir().is_none());
        assert_eq!(
            project_local_file(Path::new("/p")),
            Path::new("/p/.z-engine/config.local.toml")
        );
    }

    #[test]
    fn env_overrides_win_over_platform_dirs() {
        let var = |name: &str| (name == CONFIG_DIR_ENV).then(|| OsString::from("/custom"));
        let paths =
            Paths::discover_from(var, Some("/home/u".into()), Some("/data".into())).unwrap();
        assert_eq!(paths.config_dir, Path::new("/custom"));
        assert_eq!(paths.data_dir, Path::new("/data/z-engine"));
        assert_eq!(
            paths.claude_user_dir(),
            Some(PathBuf::from("/home/u/.claude"))
        );
        let missing = Paths::discover_from(|_| None, Some("/home/u".into()), None);
        assert!(matches!(
            missing,
            Err(ConfigError::NoDirectory { kind: "data", .. })
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn default_config_dir_matches_v1() {
        let paths =
            Paths::discover_from(|_| None, Some("/home/u".into()), Some("/d".into())).unwrap();
        assert_eq!(paths.config_dir, Path::new("/home/u/.config/z-engine"));
    }

    #[test]
    fn ensure_creates_a_default_config_that_loads_as_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::with_roots(tmp.path().join("config"), tmp.path().join("data"));
        let outcome = paths.ensure().unwrap();
        assert!(!outcome.migrated);
        assert!(paths.sessions_dir.is_dir() && paths.cache_dir.is_dir());
        let text = fs::read_to_string(&paths.user_config_file).unwrap();
        assert!(text.contains("schema = 2"));
        let loaded = load_with_env(&paths, None, &EnvOverrides::default());
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        fs::write(
            &paths.user_config_file,
            "schema = 2\n[model]\nmain = \"kept\"\n",
        )
        .unwrap();
        paths.ensure().unwrap();
        assert!(
            fs::read_to_string(&paths.user_config_file)
                .unwrap()
                .contains("kept")
        );
    }
}
