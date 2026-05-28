use std::collections::HashSet;
use std::fs;
use std::path::Path;

use tauri::Emitter;

use crate::models::{ImportResult, OpmlOutline, Subscription};
use crate::services::opml::OpmlService;
use crate::services::{StorageService, YtDlpService};
use crate::AppContext;

// ── Export commands ─────────────────────────────────────────────────

/// Exports all subscriptions as a pretty-printed JSON file.
#[tauri::command]
pub async fn export_subscriptions_json(
    path: String,
    state: tauri::State<'_, AppContext>,
) -> Result<(), String> {
    let subs =
        StorageService::load_subscriptions(&state.data_dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&subs).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Exports all subscriptions as an OPML 2.0 XML file.
#[tauri::command]
pub async fn export_subscriptions_opml(
    path: String,
    state: tauri::State<'_, AppContext>,
) -> Result<(), String> {
    let subs =
        StorageService::load_subscriptions(&state.data_dir).map_err(|e| e.to_string())?;
    let opml = OpmlService::build_opml(&subs);
    fs::write(&path, opml).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Import command ──────────────────────────────────────────────────

/// Batch import subscriptions from a list of URLs or a file path.
///
/// When `file_path` is provided:
/// - `.opml` files are parsed with `OpmlService::parse_opml()`.
/// - `.txt` files are split by newline, treating each non-empty line as a URL.
///
/// When `urls` is non-empty, those URLs are used directly.
///
/// Each URL is validated via `YtDlpService::parse_channel_info()`.
/// Duplicates (matching existing subscription URLs) are skipped.
#[tauri::command]
pub async fn batch_import_subscriptions(
    urls: Vec<String>,
    file_path: Option<String>,
    state: tauri::State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<ImportResult, String> {
    // ── Resolve URL list ────────────────────────────────────────────
    let resolved_urls: Vec<String> = if let Some(ref fp) = file_path {
        let content =
            fs::read_to_string(fp).map_err(|e| format!("Failed to read file: {}", e))?;
        let path = Path::new(fp);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "opml" => {
                let outlines = OpmlService::parse_opml(&content);
                outlines.into_iter().map(|o| o.xml_url).collect()
            }
            _ => {
                // Treat as plain text: one URL per line
                content
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            }
        }
    } else {
        urls.into_iter()
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
            .collect()
    };

    // ── Load existing subscriptions ─────────────────────────────────
    let mut subs =
        StorageService::load_subscriptions(&state.data_dir).map_err(|e| e.to_string())?;
    let existing_urls: HashSet<&str> = subs.iter().map(|s| s.url.as_str()).collect();

    // ── Read settings for yt-dlp path, proxy, and cookie_file ───────
    let (yt_dlp_path, proxy, cookie_file) = {
        let settings = state.settings.lock().map_err(|e| e.to_string())?;
        let proxy = if settings.proxy_url.is_empty() {
            None
        } else {
            Some(settings.proxy_url.clone())
        };
        let cookie_file = if settings.cookie_file.is_empty() {
            None
        } else {
            Some(settings.cookie_file.clone())
        };
        (settings.yt_dlp_path.clone(), proxy, cookie_file)
    };

    // ── Process each URL ────────────────────────────────────────────
    let total = resolved_urls.len();
    let mut imported: Vec<Subscription> = Vec::new();
    let mut skipped_duplicates: Vec<String> = Vec::new();
    let mut skipped_invalid: Vec<String> = Vec::new();

    for url in resolved_urls {
        if existing_urls.contains(url.as_str())
            || imported.iter().any(|s| s.url == url)
        {
            skipped_duplicates.push(url);
            continue;
        }

        let url_clone = url.clone();
        let yt_dlp = yt_dlp_path.clone();
        let proxy_clone = proxy.clone();
        let cookie_clone = cookie_file.clone();

        match tokio::task::spawn_blocking(move || {
            YtDlpService::parse_channel_info(&yt_dlp, &proxy_clone, &cookie_clone, &url_clone)
        })
        .await
        .map_err(|e| format!("spawn_blocking error: {}", e))?
        {
            Ok(channel_info) => {
                let sub = Subscription::new(
                    url.clone(),
                    channel_info.platform,
                    channel_info.channel_name,
                    channel_info.channel_avatar_url,
                );
                imported.push(sub);
            }
            Err(_) => {
                skipped_invalid.push(url);
            }
        }
    }

    let success_count = imported.len();

    // ── Merge and save ──────────────────────────────────────────────
    subs.extend(imported.clone());
    StorageService::save_subscriptions(&state.data_dir, &subs)
        .map_err(|e| e.to_string())?;

    // Emit event to refresh the frontend
    let _ = app_handle.emit("subscriptions-updated", ());

    Ok(ImportResult {
        imported,
        skipped_duplicates,
        skipped_invalid,
        total,
        success_count,
    })
}
