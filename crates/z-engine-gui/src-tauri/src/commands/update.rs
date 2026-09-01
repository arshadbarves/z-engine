use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

const GITHUB_RELEASES: &str = "https://api.github.com/repos/arshadbarves/z-engine/releases/latest";
const CACHE_TTL_SECS: u64 = 6 * 3600;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub current: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub phase: String,
    pub downloaded_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CachedUpdate {
    checked_at: u64,
    info: UpdateInfo,
}

fn cache_path() -> PathBuf {
    z_engine_core::config::app_data_write_dir().join("update-check.json")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn strip_v(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (
        semver::Version::parse(latest),
        semver::Version::parse(current),
    ) {
        (Ok(l), Ok(c)) => l > c,
        _ => false,
    }
}

fn read_cache() -> Option<CachedUpdate> {
    std::fs::read_to_string(cache_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn write_cache(info: &UpdateInfo) {
    let cached = CachedUpdate {
        checked_at: now_secs(),
        info: info.clone(),
    };
    if let Ok(text) = serde_json::to_string(&cached) {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, text);
    }
}

fn stale_info(current: &str) -> UpdateInfo {
    read_cache().map(|c| c.info).unwrap_or(UpdateInfo {
        available: false,
        current: current.to_string(),
        latest: None,
        url: None,
        release_notes: None,
    })
}

async fn fetch_latest(current: &str) -> UpdateInfo {
    let client = match reqwest::Client::builder()
        .user_agent(format!("z-engine-gui/{current}"))
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "update check client build failed");
            return stale_info(current);
        }
    };

    let resp = match client.get(GITHUB_RELEASES).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::warn!(status = %r.status(), "GitHub releases fetch failed");
            return stale_info(current);
        }
        Err(e) => {
            tracing::warn!(error = %e, "GitHub releases fetch failed");
            return stale_info(current);
        }
    };

    let raw: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "GitHub releases parse failed");
            return stale_info(current);
        }
    };

    let tag = raw
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let latest = strip_v(tag).to_string();
    let url = raw
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let release_notes = raw.get("body").and_then(|v| v.as_str()).map(str::to_owned);
    let available = !latest.is_empty() && is_newer(&latest, current);

    UpdateInfo {
        available,
        current: current.to_string(),
        latest: if latest.is_empty() {
            None
        } else {
            Some(latest)
        },
        url,
        release_notes,
    }
}

/// Poll GitHub for the latest release and compare against the running build.
#[tauri::command]
pub(crate) async fn check_for_update(force: Option<bool>) -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    if force != Some(true) {
        if let Some(cached) = read_cache() {
            let age = now_secs().saturating_sub(cached.checked_at);
            // Ignore cache from a different binary version (e.g. after a
            // manual install) so we never keep advertising a stale update.
            if age < CACHE_TTL_SECS && cached.info.current == current {
                return Ok(cached.info);
            }
        }
    }

    let info = fetch_latest(&current).await;
    write_cache(&info);
    Ok(info)
}

/// Open the release page in the system browser.
#[tauri::command]
pub(crate) fn open_release_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Download, install, and restart when a signed updater bundle is available.
#[tauri::command]
pub(crate) async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let update = match app.updater() {
        Ok(updater) => match updater.check().await {
            Ok(Some(update)) => update,
            Ok(None) => {
                tracing::info!("updater check: already on latest");
                return Err("no update available".into());
            }
            Err(e) => {
                tracing::warn!(error = %e, "updater check failed");
                return Err(e.to_string());
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "updater init failed");
            return Err(e.to_string());
        }
    };

    let app_handle = app.clone();
    let mut downloaded = 0u64;

    if let Err(e) = update
        .download_and_install(
            move |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                let percentage = content_length.map(|total| {
                    if total > 0 {
                        ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    }
                });
                let _ = app_handle.emit(
                    "update-progress",
                    UpdateProgress {
                        phase: "downloading".into(),
                        downloaded_bytes: downloaded,
                        total_bytes: content_length,
                        percentage,
                    },
                );
            },
            {
                let app_handle2 = app.clone();
                move || {
                    let _ = app_handle2.emit(
                        "update-progress",
                        UpdateProgress {
                            phase: "installing".into(),
                            downloaded_bytes: downloaded,
                            total_bytes: None,
                            percentage: Some(100.0),
                        },
                    );
                }
            },
        )
        .await
    {
        tracing::warn!(error = %e, "updater download/install failed");
        return Err(e.to_string());
    }

    let _ = app.emit(
        "update-progress",
        UpdateProgress {
            phase: "ready".into(),
            downloaded_bytes: downloaded,
            total_bytes: None,
            percentage: Some(100.0),
        },
    );

    app.request_restart();
    Ok(())
}

const GITHUB_CHANGELOG: &str =
    "https://raw.githubusercontent.com/arshadbarves/z-engine/release/CHANGELOG.md";
const EMBEDDED_CHANGELOG: &str = include_str!("../../../../../CHANGELOG.md");

/// Fetch full changelog markdown from GitHub (with offline embedded fallback).
#[tauri::command]
pub(crate) async fn get_changelog() -> Result<String, String> {
    let client = match reqwest::Client::builder()
        .user_agent("z-engine-gui")
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Ok(EMBEDDED_CHANGELOG.to_string()),
    };

    if let Ok(resp) = client.get(GITHUB_CHANGELOG).send().await {
        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                if !text.trim().is_empty() {
                    return Ok(text);
                }
            }
        }
    }

    Ok(EMBEDDED_CHANGELOG.to_string())
}
