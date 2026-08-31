use std::collections::BTreeMap;
use std::path::PathBuf;

// ---- model catalog (models.dev + local overrides) ---------------------------

/// Trimmed model entry for the picker.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CatalogModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    reasoning: bool,
    /// Vision / image input support.
    #[serde(default)]
    attachment: bool,
    #[serde(default)]
    context: Option<u64>,
    #[serde(default)]
    output: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CatalogProvider {
    #[serde(default)]
    name: String,
    #[serde(default)]
    models: BTreeMap<String, CatalogModel>,
}

type Catalog = BTreeMap<String, CatalogProvider>;

fn catalog_cache_path() -> PathBuf {
    z_engine_core::config::app_data_write_dir().join("models-cache.json")
}

/// Local override file in the same shape as the command output:
/// `{"providers": {"<id>": {"name": ..., "models": {"<id>": {...}}}}}`.
/// Entries are merged over the fetched catalog (fields win individually).
fn local_models_override() -> Catalog {
    let path = z_engine_core::config::models_override_path();
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Catalog>(&t).ok())
        .unwrap_or_default()
}

/// Fetch the models.dev catalog, trim to picker essentials, merge local
/// overrides, and cache on disk. Falls back to the stale cache (or just
/// the overrides) when offline.
#[tauri::command]
pub(crate) async fn fetch_model_catalog() -> Result<serde_json::Value, String> {
    const URL: &str = "https://models.dev/api.json";
    let cache = catalog_cache_path();
    let stale_ok = std::fs::metadata(&cache)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| {
            t.elapsed()
                .map(|e| e.as_secs() < 24 * 3600)
                .unwrap_or(false)
        })
        .unwrap_or(false);

    let fetched: Option<Catalog> = match reqwest::Client::new()
        .get(URL)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(raw) => Some(trim_catalog(&raw)),
            Err(e) => {
                tracing::warn!(error = %e, "models.dev response parse failed");
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "models.dev fetch failed");
            None
        }
    };

    if let Some(cat) = &fetched {
        if let Ok(text) = serde_json::to_string(cat) {
            if let Some(parent) = cache.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&cache, text);
        }
    }

    let mut merged: Catalog = match (&fetched, stale_ok) {
        (Some(c), _) => c.clone(),
        (None, true) => serde_json::from_str(&std::fs::read_to_string(&cache).unwrap_or_default())
            .unwrap_or_default(),
        _ => BTreeMap::new(),
    };
    for (pid, prov) in local_models_override() {
        let entry = merged.entry(pid).or_insert(CatalogProvider {
            name: prov.name.clone(),
            models: BTreeMap::new(),
        });
        if !prov.name.is_empty() {
            entry.name = prov.name;
        }
        for (mid, model) in prov.models {
            entry.models.insert(mid, model);
        }
    }
    merged.retain(|pid, _| pid == "openrouter");
    serde_json::to_value(&merged).map_err(|e| e.to_string())
}

/// Reduce the raw 4MB models.dev payload to what the picker shows.
fn trim_catalog(raw: &serde_json::Value) -> Catalog {
    let mut out = Catalog::new();
    let Some(providers) = raw.as_object() else {
        return out;
    };
    for (pid, pv) in providers {
        let name = pv.get("name").and_then(|v| v.as_str()).unwrap_or(pid);
        let provider = out.entry(pid.clone()).or_insert_with(|| CatalogProvider {
            name: name.to_string(),
            models: BTreeMap::new(),
        });
        if provider.name.is_empty() {
            provider.name = name.to_string();
        }
        let Some(models) = pv.get("models").and_then(|v| v.as_object()) else {
            continue;
        };
        for (mid, mv) in models {
            let limit = mv.get("limit");
            provider.models.insert(
                mid.clone(),
                CatalogModel {
                    name: mv
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(mid)
                        .to_string(),
                    reasoning: mv
                        .get("reasoning")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    attachment: mv
                        .get("attachment")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    context: limit
                        .and_then(|l| l.get("context"))
                        .and_then(|v| v.as_u64()),
                    output: limit.and_then(|l| l.get("output")).and_then(|v| v.as_u64()),
                },
            );
        }
    }
    out
}
