//! Layered settings: defaults < user < project < project-local < environment.
//!
//! Layers merge as raw TOML (see `merge.rs`) and the result is deserialized
//! after every layer, so a layer that does not fit the schema is skipped and
//! reported instead of failing the load. v1 files are migrated on the way.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::{Table, Value};
use ts_rs::TS;

use crate::document::{self, RawFile, Schema};
use crate::error::ConfigError;
use crate::merge::{merge_layer, nested};
use crate::migrate::{convert_v1, persist};
use crate::paths::{Paths, project_config_file, project_local_file};
use crate::settings::{Settings, normalize, unknown_keys};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub enum LayerScope {
    Default,
    User,
    Project,
    ProjectLocal,
    Env,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct LayerInfo {
    pub scope: LayerScope,
    pub path: Option<PathBuf>,
    /// The file exists; for the env layer, an override variable is set.
    pub exists: bool,
    /// Why the layer was skipped.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct LoadedSettings {
    pub settings: Settings,
    /// Lowest precedence first.
    pub layers: Vec<LayerInfo>,
    pub warnings: Vec<String>,
}

/// Environment variables that override every file layer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvOverrides {
    /// `ZENGINE_MODEL` -> `model.main`
    pub model: Option<String>,
    /// `ZENGINE_BASE_URL` -> `provider.base_url`
    pub base_url: Option<String>,
    /// `ZENGINE_PROVIDER` -> `provider.kind`
    pub provider: Option<String>,
    /// `ZENGINE_SHELL` -> `shell.path` (honored by v1)
    pub shell: Option<String>,
}

impl EnvOverrides {
    pub fn from_process() -> Self {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        Self {
            model: var("ZENGINE_MODEL"),
            base_url: var("ZENGINE_BASE_URL"),
            provider: var("ZENGINE_PROVIDER"),
            shell: var("ZENGINE_SHELL"),
        }
    }

    fn table(&self) -> Table {
        let overrides = [
            (["model", "main"], &self.model),
            (["provider", "base_url"], &self.base_url),
            (["provider", "kind"], &self.provider),
            (["shell", "path"], &self.shell),
        ];
        let mut table = Table::new();
        for (path, value) in overrides {
            if let Some(value) = value {
                merge_layer(&mut table, nested(&path, Value::String(value.clone())));
            }
        }
        table
    }
}

/// Loads with the process environment's overrides. Never fails: problems
/// are reported per layer and in `warnings`.
pub fn load(paths: &Paths, project_root: Option<&Path>) -> LoadedSettings {
    load_with_env(paths, project_root, &EnvOverrides::from_process())
}

pub fn load_with_env(
    paths: &Paths,
    project_root: Option<&Path>,
    env: &EnvOverrides,
) -> LoadedSettings {
    let mut layering = Layering::default();
    layering.layers.push(LayerInfo {
        scope: LayerScope::Default,
        path: None,
        exists: true,
        error: None,
    });
    layering.file(LayerScope::User, &paths.user_config_file);
    if let Some(root) = project_root {
        layering.file(LayerScope::Project, &project_config_file(root));
        layering.file(LayerScope::ProjectLocal, &project_local_file(root));
    }
    let table = env.table();
    let exists = !table.is_empty();
    let error = if exists {
        layering.apply(table).err()
    } else {
        None
    };
    if let Some(error) = &error {
        layering
            .warnings
            .push(format!("environment overrides were skipped: {error}"));
    }
    layering.layers.push(LayerInfo {
        scope: LayerScope::Env,
        path: None,
        exists,
        error,
    });
    layering.finish()
}

#[derive(Default)]
struct Layering {
    merged: Table,
    settings: Settings,
    layers: Vec<LayerInfo>,
    warnings: Vec<String>,
}

impl Layering {
    fn file(&mut self, scope: LayerScope, path: &Path) {
        let mut info = LayerInfo {
            scope,
            path: Some(path.to_path_buf()),
            exists: false,
            error: None,
        };
        match self.read(path) {
            Ok(None) => {}
            Ok(Some(table)) => {
                info.exists = true;
                info.error = self.apply(table).err();
            }
            Err(error) => {
                info.exists = true;
                info.error = Some(without_path(error));
            }
        }
        if let Some(error) = &info.error {
            self.warnings
                .push(format!("{} was skipped: {error}", path.display()));
        }
        self.layers.push(info);
    }

    fn read(&mut self, path: &Path) -> Result<Option<Table>, ConfigError> {
        let table = {
            let _guard = document::lock();
            let Some(raw) = document::read(path)? else {
                return Ok(None);
            };
            match document::schema_of(&raw.table) {
                Schema::V1 => self.migrate(path, raw),
                Schema::Invalid => return Err(document::invalid_schema(path)),
                Schema::Newer(version) => {
                    self.warnings.push(format!(
                        "{}: written for settings schema {version}; unknown keys are ignored",
                        path.display()
                    ));
                    raw.table
                }
                Schema::Current => raw.table,
            }
        };
        let shown = path.display();
        let unknown = unknown_keys(&table).into_iter();
        self.warnings
            .extend(unknown.map(|key| format!("{shown}: unknown key `{key}`")));
        Ok(Some(table))
    }

    /// Converts a v1 layer and saves the result; if saving fails the
    /// converted table is still used for this load.
    fn migrate(&mut self, path: &Path, raw: RawFile) -> Table {
        let (table, notes) = convert_v1(raw.table);
        let shown = path.display();
        match persist(path, &raw.text, &table) {
            Ok(backup) => self.warnings.push(format!(
                "{shown}: migrated from v1; the original is saved as {}",
                backup.display()
            )),
            Err(error) => self.warnings.push(format!(
                "{shown}: read as v1 settings; the migrated file could not be saved: {error}"
            )),
        }
        self.warnings
            .extend(notes.into_iter().map(|note| format!("{shown}: {note}")));
        table
    }

    fn apply(&mut self, layer: Table) -> Result<(), String> {
        let mut candidate = self.merged.clone();
        merge_layer(&mut candidate, layer);
        self.settings = document::deserialize(candidate.clone())?;
        self.merged = candidate;
        Ok(())
    }

    fn finish(mut self) -> LoadedSettings {
        self.warnings.extend(normalize(&mut self.settings));
        LoadedSettings {
            settings: self.settings,
            layers: self.layers,
            warnings: self.warnings,
        }
    }
}

/// The reason a layer file failed, without the path its layer records.
fn without_path(error: ConfigError) -> String {
    match error {
        ConfigError::Parse { message, .. } => message,
        ConfigError::Io { source, .. } => source.to_string(),
        ConfigError::TooLarge { limit, .. } => format!("larger than {limit} bytes"),
        other => other.to_string(),
    }
}
