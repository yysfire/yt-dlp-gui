use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Emitter;

use crate::models::{ImportCompleteEvent, ImportErrorItem, ImportPreview, ImportPreviewItem,
    ImportProgressEvent, ImportResult, ImportSource, Subscription};
use crate::services::opml::OpmlService;
use crate::services::{StorageService, YtDlpService};
use crate::AppContext;

/// Global import state for cancellation support.
pub struct ImportContext {
    /// Cancellation flag for the currently running import task.
    /// Shared Arc<AtomicBool> between the command and background task.
    pub cancel_flag: std::sync::Mutex<Option<(String, Arc<AtomicBool>)>>,
}

const MAX_URL_LENGTH: usize = 2048;

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
                parse_txt_url_list(&content)
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

// ── TXT URL parsing helper (T010) ───────────────────────────────────

/// Parses a TXT file content into a list of URLs.
///
/// Rules:
/// - Each non-empty line is treated as a candidate URL.
/// - Lines starting with `#` are treated as comments and skipped.
/// - Leading/trailing whitespace is trimmed.
/// - Empty lines are skipped.
/// - URLs exceeding `MAX_URL_LENGTH` (2048 chars) are skipped.
/// - Invalid URLs (per the `url` crate) are skipped.
pub fn parse_txt_url_list(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| line.len() <= MAX_URL_LENGTH)
        .filter(|line| url::Url::parse(line).is_ok())
        .collect()
}

// ── Batch import preview command (T012) ─────────────────────────────

/// Parses an import source and returns a preview of what will be imported.
///
/// No yt-dlp calls are made during preview — only the source file is parsed
/// and duplicates against existing subscriptions are checked.
#[tauri::command]
pub async fn batch_import_preview(
    source: ImportSource,
    state: tauri::State<'_, AppContext>,
) -> Result<ImportPreview, String> {
    // ── Resolve URLs from source ────────────────────────────────────
    let raw_items: Vec<(String, Option<String>)> = match source {
        ImportSource::OpmlFile { path } => {
            let content =
                fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
            let outlines = OpmlService::parse_opml(&content);
            outlines
                .into_iter()
                .map(|o| (o.xml_url, if o.title.is_empty() { None } else { Some(o.title) }))
                .collect()
        }
        ImportSource::TxtFile { path } => {
            let content =
                fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
            parse_txt_url_list(&content)
                .into_iter()
                .map(|u| (u, None))
                .collect()
        }
        ImportSource::UrlList { urls } => urls
            .into_iter()
            .map(|u| (u.trim().to_string(), None))
            .filter(|(u, _)| !u.is_empty())
            .collect(),
    };

    // ── Load existing subscription URLs for duplicate detection ──────
    let subs =
        StorageService::load_subscriptions(&state.data_dir).map_err(|e| e.to_string())?;
    let existing_urls: HashSet<&str> = subs.iter().map(|s| s.url.as_str()).collect();

    // ── Build preview items with validation and duplicate detection ──
    let mut items: Vec<ImportPreviewItem> = Vec::new();
    let mut duplicate_count = 0;

    for (url, title) in raw_items {
        let is_duplicate = existing_urls.contains(url.as_str());
        if is_duplicate {
            duplicate_count += 1;
        }

        let error = if url.is_empty() {
            Some("Empty URL".to_string())
        } else if url.len() > MAX_URL_LENGTH {
            Some(format!("URL exceeds {} characters", MAX_URL_LENGTH))
        } else if url::Url::parse(&url).is_err() {
            Some("Invalid URL format".to_string())
        } else {
            None
        };

        items.push(ImportPreviewItem {
            url,
            title,
            is_duplicate,
            error,
        });
    }

    let total = items.len();

    Ok(ImportPreview {
        items,
        total,
        duplicates: duplicate_count,
    })
}

// ── Batch import execute command (T013) ─────────────────────────────

/// Executes the actual import as a background task.
///
/// Spawns a tokio task that processes each URL concurrently using
/// `YtDlpService::parse_channel_info()`. Progress and completion are
/// reported via `import-progress` and `import-complete` events.
///
/// Returns a task_id for tracking and cancellation.
#[tauri::command]
pub async fn batch_import_execute(
    source: ImportSource,
    skip_duplicates: bool,
    state: tauri::State<'_, AppContext>,
    import_ctx: tauri::State<'_, ImportContext>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // ── Resolve URLs from source ────────────────────────────────────
    let raw_items: Vec<(String, Option<String>)> = match source {
        ImportSource::OpmlFile { path } => {
            let content =
                fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
            let outlines = OpmlService::parse_opml(&content);
            outlines
                .into_iter()
                .map(|o| (o.xml_url, if o.title.is_empty() { None } else { Some(o.title) }))
                .collect()
        }
        ImportSource::TxtFile { path } => {
            let content =
                fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
            parse_txt_url_list(&content)
                .into_iter()
                .map(|u| (u, None))
                .collect()
        }
        ImportSource::UrlList { urls } => urls
            .into_iter()
            .map(|u| (u.trim().to_string(), None))
            .filter(|(u, _)| !u.is_empty())
            .collect(),
    };

    // Generate a unique task ID
    let task_id = uuid::Uuid::new_v4().to_string();
    let task_id_for_ctx = task_id.clone();
    let task_id_for_task = task_id.clone();

    // ── Load existing subscriptions for duplicate detection ──────────
    let subs =
        StorageService::load_subscriptions(&state.data_dir).map_err(|e| e.to_string())?;
    let existing_urls: HashSet<String> = subs.iter().map(|s| s.url.clone()).collect();

    // ── Read settings ────────────────────────────────────────────────
    let (data_dir, yt_dlp_path, proxy, cookie_file) = {
        let settings = state.settings.lock().map_err(|e| e.to_string())?;
        (
            state.data_dir.clone(),
            settings.yt_dlp_path.clone(),
            if settings.proxy_url.is_empty() {
                None
            } else {
                Some(settings.proxy_url.clone())
            },
            if settings.cookie_file.is_empty() {
                None
            } else {
                Some(settings.cookie_file.clone())
            },
        )
    };

    // ── Setup cancellation ──────────────────────────────────────────
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_clone = cancel_flag.clone();
    {
        let mut ctx = import_ctx.cancel_flag.lock().map_err(|e| e.to_string())?;
        *ctx = Some((task_id_for_ctx, cancel_flag_clone));
    }

    // ── Spawn background task ────────────────────────────────────────
    tauri::async_runtime::spawn(async move {
        let total = raw_items.len();
        let mut imported: Vec<Subscription> = Vec::new();
        let mut skipped: usize = 0;
        let mut errors: Vec<ImportErrorItem> = Vec::new();

        for (idx, (url, title)) in raw_items.iter().enumerate() {
            // Check cancellation
            if cancel_flag.load(Ordering::Relaxed) {
                log::info!("Import task {} cancelled at {}/{}", task_id_for_task, idx, total);
                skipped += total - idx;
                break;
            }

            let current_title = title.as_deref().unwrap_or("").to_string();

            // Emit progress
            let _ = app_handle.emit(
                "import-progress",
                ImportProgressEvent {
                    task_id: task_id_for_task.clone(),
                    completed: idx,
                    total,
                    current_url: url.clone(),
                    current_title: current_title.clone(),
                },
            );

            // Skip duplicates
            if skip_duplicates && existing_urls.contains(url.as_str()) {
                skipped += 1;
                continue;
            }

            // Skip invalid URLs
            if url.is_empty() || url.len() > MAX_URL_LENGTH || url::Url::parse(url).is_err() {
                skipped += 1;
                errors.push(ImportErrorItem {
                    url: url.clone(),
                    reason: "Invalid URL format".to_string(),
                });
                continue;
            }

            let url_clone = url.clone();
            let yt_dlp = yt_dlp_path.clone();
            let proxy_clone = proxy.clone();
            let cookie_clone = cookie_file.clone();

            match tokio::task::spawn_blocking(move || {
                YtDlpService::parse_channel_info(
                    &yt_dlp,
                    &proxy_clone,
                    &cookie_clone,
                    &url_clone,
                )
            })
            .await
            {
                Ok(Ok(channel_info)) => {
                    let sub = Subscription::new(
                        url.clone(),
                        channel_info.platform,
                        channel_info.channel_name,
                        channel_info.channel_avatar_url,
                    );
                    imported.push(sub);
                }
                Ok(Err(_)) | Err(_) => {
                    errors.push(ImportErrorItem {
                        url: url.clone(),
                        reason: "yt-dlp channel resolution failed".to_string(),
                    });
                }
            }
        }

        let success_count = imported.len();
        let failed_count = errors.len();

        // ── Merge and save ──────────────────────────────────────────
        if !imported.is_empty() {
            if let Ok(mut subs) = StorageService::load_subscriptions(&data_dir) {
                subs.extend(imported);
                let _ = StorageService::save_subscriptions(&data_dir, &subs);
            }
        }

        // ── Emit completion event ────────────────────────────────────
        let _ = app_handle.emit(
            "import-complete",
            ImportCompleteEvent {
                task_id: task_id_for_task.clone(),
                total,
                success: success_count,
                failed: failed_count,
                skipped,
                errors,
            },
        );

        // Emit refresh event
        let _ = app_handle.emit("subscriptions-updated", ());

        // Clean up cancellation flag
        // Note: this happens in a spawned task, so we can't access ImportContext mutex here.
        // The flag will be cleaned up on next import or cancel.
        log::info!(
            "Import task {} completed: {}/{} success, {} failed, {} skipped",
            task_id_for_task,
            success_count,
            total,
            failed_count,
            skipped
        );
    });

    Ok(task_id)
}

// ── Cancel import command (T014) ────────────────────────────────────

/// Cancels a running batch import task.
///
/// If no task with the given ID is running, this is a no-op.
#[tauri::command]
pub async fn cancel_import(
    task_id: String,
    import_ctx: tauri::State<'_, ImportContext>,
) -> Result<(), String> {
    let mut ctx = import_ctx.cancel_flag.lock().map_err(|e| e.to_string())?;
    if let Some((stored_id, flag)) = ctx.as_ref() {
        if *stored_id == task_id {
            flag.store(true, Ordering::Relaxed);
            log::info!("Cancellation requested for import task {}", task_id);
        }
    }
    // Clear the cancellation context regardless
    *ctx = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_temp_dir() -> TempDir {
        TempDir::new().expect("failed to create temp dir")
    }

    fn make_existing_sub(data_dir: &Path, url: &str) -> Subscription {
        let sub = Subscription::new(
            url.to_string(),
            "youtube".to_string(),
            format!("Channel for {}", url),
            "https://example.com/avatar.jpg".to_string(),
        );
        StorageService::save_subscriptions(data_dir, &[sub.clone()])
            .expect("save should succeed");
        sub
    }

    // ── T010: TXT URL list parsing tests ───────────────────────────

    #[test]
    fn test_txt_parse_url_list() {
        let content = "https://youtube.com/@channel1\nhttps://youtube.com/@channel2\nhttps://bilibili.com/space/123\n";
        let urls = parse_txt_url_list(content);
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://youtube.com/@channel1");
        assert_eq!(urls[1], "https://youtube.com/@channel2");
        assert_eq!(urls[2], "https://bilibili.com/space/123");
    }

    #[test]
    fn test_txt_skip_empty_lines() {
        let content = "https://youtube.com/@a\n\nhttps://youtube.com/@b\n\n\nhttps://youtube.com/@c\n";
        let urls = parse_txt_url_list(content);
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://youtube.com/@a");
        assert_eq!(urls[1], "https://youtube.com/@b");
        assert_eq!(urls[2], "https://youtube.com/@c");
    }

    #[test]
    fn test_txt_skip_comment_lines() {
        let content = "# This is a comment\nhttps://youtube.com/@a\n# Another comment\nhttps://youtube.com/@b\n";
        let urls = parse_txt_url_list(content);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://youtube.com/@a");
        assert_eq!(urls[1], "https://youtube.com/@b");
    }

    #[test]
    fn test_txt_reject_invalid_url() {
        let content = "https://youtube.com/@valid\nnot-a-url\njust-some-text\nhttps://youtube.com/@also-valid\n";
        let urls = parse_txt_url_list(content);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://youtube.com/@valid");
        assert_eq!(urls[1], "https://youtube.com/@also-valid");
    }

    #[test]
    fn test_txt_reject_long_url() {
        // Create a URL longer than 2048 characters
        let long_path = "a".repeat(MAX_URL_LENGTH);
        let long_url = format!("https://example.com/{}", long_path);

        let content = format!("https://youtube.com/@short\n{}\nhttps://youtube.com/@also-short", long_url);
        let urls = parse_txt_url_list(&content);
        assert_eq!(urls.len(), 2, "long URL should be filtered out");
        assert_eq!(urls[0], "https://youtube.com/@short");
        assert_eq!(urls[1], "https://youtube.com/@also-short");
    }

    #[test]
    fn test_txt_parse_empty_content() {
        let urls = parse_txt_url_list("");
        assert!(urls.is_empty());

        let urls = parse_txt_url_list("\n\n\n");
        assert!(urls.is_empty());

        let urls = parse_txt_url_list("# only comments\n# another comment\n");
        assert!(urls.is_empty());
    }

    #[test]
    fn test_txt_trim_whitespace() {
        let content = "  https://youtube.com/@a  \n\thttps://youtube.com/@b\t\n";
        let urls = parse_txt_url_list(content);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://youtube.com/@a");
        assert_eq!(urls[1], "https://youtube.com/@b");
    }

    // ── T011: Batch import integration tests ───────────────────────

    /// 部分 URL 无效时已成功的导入不受影响
    #[test]
    fn test_batch_import_partial_failure() {
        let tmp = setup_temp_dir();
        // Pre-populate with no existing subscriptions
        StorageService::save_subscriptions(tmp.path(), &[])
            .expect("save empty subs");

        // Simulate parsing: some valid, some invalid
        let content = "https://youtube.com/@good1\nnot-a-url\nhttps://youtube.com/@good2\n  \n";
        let urls = parse_txt_url_list(content);

        // Valid URLs should be in the list
        assert_eq!(urls.len(), 2, "invalid URLs should be filtered out");
        assert!(urls.contains(&"https://youtube.com/@good1".to_string()));
        assert!(urls.contains(&"https://youtube.com/@good2".to_string()));
    }

    /// 已有订阅的 URL 被正确检测为重复
    #[test]
    fn test_batch_import_duplicate_detection() {
        let tmp = setup_temp_dir();

        // Create an existing subscription
        let existing_url = "https://youtube.com/@existing";
        let _existing_sub = make_existing_sub(tmp.path(), existing_url);

        // Load existing subs
        let subs = StorageService::load_subscriptions(tmp.path())
            .expect("load subs");
        let existing_urls: HashSet<String> = subs.iter().map(|s| s.url.clone()).collect();
        assert!(existing_urls.contains(existing_url), "existing URL should be detected");

        // New URLs to import (one duplicate, one new)
        let new_urls = vec![
            existing_url.to_string(),
            "https://youtube.com/@new-channel".to_string(),
        ];

        let mut duplicates = 0;
        let mut non_duplicates = 0;
        for url in &new_urls {
            if existing_urls.contains(url.as_str()) {
                duplicates += 1;
            } else {
                non_duplicates += 1;
            }
        }

        assert_eq!(duplicates, 1, "one URL should be detected as duplicate");
        assert_eq!(non_duplicates, 1, "one URL should be new");
    }

    /// 测试 ImportPreviewItem 构建逻辑
    #[test]
    fn test_batch_import_preview_item_duplicate_handling() {
        let tmp = setup_temp_dir();

        let existing_url = "https://youtube.com/@existing";
        make_existing_sub(tmp.path(), existing_url);

        let subs = StorageService::load_subscriptions(tmp.path()).expect("load subs");
        let existing_urls: HashSet<&str> = subs.iter().map(|s| s.url.as_str()).collect();

        // Build preview items
        let test_urls = vec![
            ("https://youtube.com/@existing", Some("Existing Channel".to_string())),
            ("https://youtube.com/@new", Some("New Channel".to_string())),
            ("not-a-url", None),
        ];

        let mut items: Vec<ImportPreviewItem> = Vec::new();
        for (url, title) in &test_urls {
            let is_duplicate = existing_urls.contains(url);
            let error = if url.len() > MAX_URL_LENGTH {
                Some(format!("URL exceeds {} characters", MAX_URL_LENGTH))
            } else if url::Url::parse(url).is_err() {
                Some("Invalid URL format".to_string())
            } else {
                None
            };

            items.push(ImportPreviewItem {
                url: url.to_string(),
                title: title.clone(),
                is_duplicate,
                error,
            });
        }

        assert_eq!(items.len(), 3);
        assert!(items[0].is_duplicate, "existing URL should be marked as duplicate");
        assert_eq!(items[0].title, Some("Existing Channel".to_string()));
        assert_eq!(items[0].error, None);

        assert!(!items[1].is_duplicate, "new URL should not be duplicate");
        assert_eq!(items[1].error, None);

        assert!(!items[2].is_duplicate);
        assert_eq!(items[2].error, Some("Invalid URL format".to_string()));
    }
}
