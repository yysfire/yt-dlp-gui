use std::collections::HashMap;
use std::path::Path;

use crate::models::{AppSettings, AppState, DownloadRecord, Subscription};
use crate::utils::AppError;

/// Stateless service for reading/writing JSON persistence files.
pub struct StorageService;

impl StorageService {
    // ── Subscriptions ──────────────────────────────────────────────

    /// Loads all subscriptions from `subscriptions.json`.
    /// Returns an empty Vec if the file does not exist.
    pub fn load_subscriptions(data_dir: &Path) -> Result<Vec<Subscription>, AppError> {
        let path = data_dir.join("subscriptions.json");
        Self::read_json_array(&path)
    }

    /// Saves the full subscription list to `subscriptions.json`.
    pub fn save_subscriptions(
        data_dir: &Path,
        subs: &[Subscription],
    ) -> Result<(), AppError> {
        let path = data_dir.join("subscriptions.json");
        Self::write_json(&path, subs)
    }

    // ── Download Records ───────────────────────────────────────────

    /// Loads all download records from `download_records.json`.
    pub fn load_download_records(data_dir: &Path) -> Result<Vec<DownloadRecord>, AppError> {
        let path = data_dir.join("download_records.json");
        Self::read_json_array(&path)
    }

    /// Saves the full download record list to `download_records.json`.
    pub fn save_download_records(
        data_dir: &Path,
        records: &[DownloadRecord],
    ) -> Result<(), AppError> {
        let path = data_dir.join("download_records.json");
        Self::write_json(&path, records)
    }

    /// Deduplicate a vec of DownloadRecords in memory (no disk I/O).
    /// Same priority rules as deduplicate_records().
    pub fn deduplicate_vec(records: Vec<DownloadRecord>) -> Vec<DownloadRecord> {
        let status_rank = |s: &str| -> usize {
            match s {
                "completed" => 0,
                "downloading" => 1,
                "failed" => 2,
                "paused" => 3,
                "cancelled" => 4,
                _ => 9,
            }
        };

        let mut groups: HashMap<String, Vec<DownloadRecord>> =
            HashMap::new();
        let mut no_id_records: Vec<DownloadRecord> = Vec::new();

        for r in records {
            if r.video_id.is_empty() {
                no_id_records.push(r);
            } else {
                groups.entry(r.video_id.clone()).or_default().push(r);
            }
        }

        let mut deduped: Vec<DownloadRecord> = Vec::new();
        for (_vid, mut group) in groups {
            if group.len() == 1 {
                deduped.push(group.pop().unwrap());
            } else {
                group.sort_by(|a, b| {
                    let ra = status_rank(&a.status);
                    let rb = status_rank(&b.status);
                    ra.cmp(&rb).then_with(|| b.downloaded_at.cmp(&a.downloaded_at))
                });
                deduped.push(group.remove(0));
            }
        }
        deduped.extend(no_id_records);
        deduped
    }

    /// Deduplicates download records by video_id.
    /// Priority: completed > downloading > failed > paused > cancelled.
    /// Within the same status, keeps the one with the latest downloaded_at.
    /// Records without a video_id are kept as-is.
    /// Returns the number of duplicate records removed.
    pub fn deduplicate_records(data_dir: &Path) -> Result<usize, AppError> {
        let records = Self::load_download_records(data_dir)?;
        let original_count = records.len();
        let deduped = Self::deduplicate_vec(records);
        let removed = original_count - deduped.len();
        if removed > 0 {
            log::info!(
                "Deduplicated download records: removed {} duplicate(s), {} → {} records",
                removed,
                original_count,
                deduped.len()
            );
            Self::save_download_records(data_dir, &deduped)?;
        }

        Ok(removed)
    }

    /// Recomputes total_downloads from actual download records and syncs state.json.
    /// Fixes any discrepancy caused by race conditions or interrupted writes.
    pub fn recompute_total_downloads(data_dir: &Path) -> Result<u32, AppError> {
        let records = Self::load_download_records(data_dir)?;
        let actual = records.iter().filter(|r| r.status == "completed").count() as u32;
        if let Ok(mut state) = Self::load_state(data_dir) {
            if state.total_downloads != actual {
                log::info!(
                    "Recomputing total_downloads: {} → {} (from {} records, {} completed)",
                    state.total_downloads,
                    actual,
                    records.len(),
                    actual
                );
                state.total_downloads = actual;
                Self::save_state(data_dir, &state)?;
            }
        }
        Ok(actual)
    }

    /// Recomputes per-subscription download_count from actual completed records.
    pub fn recompute_subscription_download_counts(data_dir: &Path) -> Result<(), AppError> {
        let mut subs = Self::load_subscriptions(data_dir)?;
        let records = Self::load_download_records(data_dir)?;
        let mut changed = false;

        for sub in subs.iter_mut() {
            let count = records
                .iter()
                .filter(|r| r.subscription_id == sub.id && r.status == "completed")
                .count() as u32;
            if sub.download_count != count {
                log::info!(
                    "Recomputing download_count for {}: {} → {}",
                    sub.channel_name,
                    sub.download_count,
                    count
                );
                sub.download_count = count;
                changed = true;
            }
        }

        if changed {
            Self::save_subscriptions(data_dir, &subs)?;
        }
        Ok(())
    }

    // ── Settings ────────────────────────────────────────────────────

    /// Loads settings from `settings.json`. Returns defaults if the file
    /// does not exist or is corrupt.
    pub fn load_settings(data_dir: &Path) -> AppSettings {
        let path = data_dir.join("settings.json");
        Self::read_json(&path).unwrap_or_default()
    }

    /// Saves settings to `settings.json`.
    pub fn save_settings(data_dir: &Path, settings: &AppSettings) -> Result<(), AppError> {
        let path = data_dir.join("settings.json");
        Self::write_json(&path, settings)
    }

    // ── App State ───────────────────────────────────────────────────

    /// Loads application state from `state.json`. Returns defaults if absent.
    pub fn load_state(data_dir: &Path) -> Result<AppState, AppError> {
        let path = data_dir.join("state.json");
        Ok(Self::read_json(&path).unwrap_or_default())
    }

    /// Saves application state to `state.json`.
    pub fn save_state(data_dir: &Path, state: &AppState) -> Result<(), AppError> {
        let path = data_dir.join("state.json");
        Self::write_json(&path, state)
    }

    // ── Internal helpers ────────────────────────────────────────────

    /// Reads and deserializes a JSON file into a single value.
    fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, AppError> {
        let data = std::fs::read_to_string(path)?;
        let value = serde_json::from_str(&data)?;
        Ok(value)
    }

    /// Reads and deserializes a JSON array file. Returns an empty Vec if
    /// the file is missing (first-run scenario).
    fn read_json_array<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>, AppError> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        Self::read_json(path)
    }

    /// Serializes a value to JSON and writes it to the given path.
    fn write_json<T: serde::Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), AppError> {
        let json = serde_json::to_string_pretty(value)?;
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_temp_dir() -> TempDir {
        TempDir::new().expect("failed to create temp dir")
    }

    fn make_sub(url: &str, name: &str) -> Subscription {
        Subscription::new(
            url.to_string(),
            "youtube".to_string(),
            name.to_string(),
            "https://example.com/avatar.jpg".to_string(),
        )
    }

    fn make_record(sub_id: &str, title: &str) -> DownloadRecord {
        DownloadRecord::new(
            sub_id.to_string(),
            title.to_string(),
            "https://example.com/video".to_string(),
            "".to_string(),
        )
    }

    // ── Subscriptions storage tests ───────────────────────────────

    #[test]
    fn test_subscriptions_empty_dir_returns_empty_vec() {
        let tmp = setup_temp_dir();
        let result = StorageService::load_subscriptions(tmp.path());
        assert!(result.is_ok(), "should not error on missing file");
        let subs = result.unwrap();
        assert!(subs.is_empty(), "should return empty vec for empty dir");
    }

    #[test]
    fn test_subscriptions_save_and_load_roundtrip() {
        let tmp = setup_temp_dir();
        let sub = make_sub("https://youtube.com/@test", "Test Channel");

        // Save
        StorageService::save_subscriptions(tmp.path(), &[sub.clone()])
            .expect("save should succeed");

        // Load and verify
        let loaded = StorageService::load_subscriptions(tmp.path())
            .expect("load should succeed");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, sub.id);
        assert_eq!(loaded[0].url, sub.url);
        assert_eq!(loaded[0].platform, sub.platform);
        assert_eq!(loaded[0].channel_name, sub.channel_name);
        assert_eq!(loaded[0].channel_avatar_url, sub.channel_avatar_url);
        assert_eq!(loaded[0].paused, sub.paused);
        assert_eq!(loaded[0].quality_preset, sub.quality_preset);
        assert_eq!(loaded[0].created_at, sub.created_at);
    }

    #[test]
    fn test_subscriptions_multiple_save_and_load() {
        let tmp = setup_temp_dir();
        let sub1 = make_sub("https://youtube.com/@a", "A");
        let sub2 = make_sub("https://youtube.com/@b", "B");
        let sub3 = make_sub("https://bilibili.com/space/1", "C");

        StorageService::save_subscriptions(tmp.path(), &[sub1.clone(), sub2.clone(), sub3.clone()])
            .expect("save should succeed");

        let loaded = StorageService::load_subscriptions(tmp.path())
            .expect("load should succeed");
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].id, sub1.id);
        assert_eq!(loaded[1].id, sub2.id);
        assert_eq!(loaded[2].id, sub3.id);
    }

    #[test]
    fn test_subscriptions_overwrite_on_resave() {
        let tmp = setup_temp_dir();
        let sub1 = make_sub("https://youtube.com/@a", "A");

        StorageService::save_subscriptions(tmp.path(), &[sub1.clone()])
            .expect("first save should succeed");

        let sub2 = make_sub("https://youtube.com/@b", "B");
        StorageService::save_subscriptions(tmp.path(), &[sub2.clone()])
            .expect("second save should succeed");

        let loaded = StorageService::load_subscriptions(tmp.path())
            .expect("load should succeed");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, sub2.id, "overwrite should replace old data");
    }

    #[test]
    fn test_subscriptions_empty_json_array_loads_empty_vec() {
        let tmp = setup_temp_dir();
        let file_path = tmp.path().join("subscriptions.json");
        std::fs::write(&file_path, "[]").expect("write should succeed");

        let loaded = StorageService::load_subscriptions(tmp.path())
            .expect("load should succeed");
        assert!(loaded.is_empty());
    }

    // ── Download records storage tests ────────────────────────────

    #[test]
    fn test_download_records_empty_dir_returns_empty_vec() {
        let tmp = setup_temp_dir();
        let result = StorageService::load_download_records(tmp.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_download_records_save_and_load_roundtrip() {
        let tmp = setup_temp_dir();
        let record = make_record("sub-id-1", "Test Video");

        StorageService::save_download_records(tmp.path(), &[record.clone()])
            .expect("save should succeed");

        let loaded = StorageService::load_download_records(tmp.path())
            .expect("load should succeed");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, record.id);
        assert_eq!(loaded[0].subscription_id, record.subscription_id);
        assert_eq!(loaded[0].video_title, record.video_title);
        assert_eq!(loaded[0].video_url, record.video_url);
        assert_eq!(loaded[0].file_path, record.file_path);
        assert_eq!(loaded[0].file_size, record.file_size);
        assert_eq!(loaded[0].status, record.status);
        assert_eq!(loaded[0].downloaded_at, record.downloaded_at);
    }

    #[test]
    fn test_download_records_cascade_delete_simulation() {
        let tmp = setup_temp_dir();
        let sub_id_a = "sub-a";
        let sub_id_b = "sub-b";

        let r1 = make_record(sub_id_a, "Video A1");
        let r2 = make_record(sub_id_a, "Video A2");
        let r3 = make_record(sub_id_b, "Video B1");

        StorageService::save_download_records(tmp.path(), &[r1, r2, r3])
            .expect("save should succeed");

        // Simulate cascade delete: remove records for sub_id_a
        let mut records = StorageService::load_download_records(tmp.path())
            .expect("load should succeed");
        records.retain(|r| r.subscription_id != sub_id_a);
        StorageService::save_download_records(tmp.path(), &records)
            .expect("save after retention should succeed");

        let loaded = StorageService::load_download_records(tmp.path())
            .expect("load should succeed");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].subscription_id, sub_id_b);
        assert_eq!(loaded[0].video_title, "Video B1");
    }

    #[test]
    fn test_deduplicate_vec_keeps_completed_over_failed() {
        let mut r1 = make_record("sub-1", "Video");
        r1.video_id = "vid-1".to_string();
        r1.status = "failed".to_string();
        r1.downloaded_at = "2026-06-01T12:00:00Z".to_string();

        let mut r2 = make_record("sub-1", "Video");
        r2.video_id = "vid-1".to_string();
        r2.status = "completed".to_string();
        r2.downloaded_at = "2026-06-01T11:00:00Z".to_string();

        let result = StorageService::deduplicate_vec(vec![r1.clone(), r2.clone()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, "completed");
    }

    #[test]
    fn test_deduplicate_vec_same_status_keeps_latest() {
        let mut r1 = make_record("sub-1", "Video");
        r1.video_id = "vid-1".to_string();
        r1.status = "failed".to_string();
        r1.downloaded_at = "2026-06-01T12:00:00Z".to_string();

        let mut r2 = make_record("sub-1", "Video");
        r2.video_id = "vid-1".to_string();
        r2.status = "failed".to_string();
        r2.downloaded_at = "2026-06-01T13:00:00Z".to_string();

        let result = StorageService::deduplicate_vec(vec![r1.clone(), r2.clone()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].downloaded_at, "2026-06-01T13:00:00Z");
    }

    #[test]
    fn test_deduplicate_vec_single_record_unchanged() {
        let mut r = make_record("sub-1", "Video");
        r.video_id = "vid-1".to_string();
        let result = StorageService::deduplicate_vec(vec![r.clone()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].video_id, "vid-1");
    }

    #[test]
    fn test_deduplicate_vec_empty_id_kept() {
        let r1 = make_record("sub-1", "Video A");
        let r2 = make_record("sub-1", "Video B");
        let result = StorageService::deduplicate_vec(vec![r1.clone(), r2.clone()]);
        assert_eq!(result.len(), 2);
    }

    // ── Settings storage tests ─────────────────────────────────────

    #[test]
    fn test_settings_empty_dir_returns_defaults() {
        let tmp = setup_temp_dir();
        let settings = StorageService::load_settings(tmp.path());
        // Should return defaults when no file exists
        assert_eq!(settings.check_interval_minutes, 60);
        assert_eq!(settings.yt_dlp_path, "yt-dlp");
    }

    #[test]
    fn test_settings_save_and_load_roundtrip() {
        let tmp = setup_temp_dir();
        let settings = AppSettings {
            download_dir: "/custom/downloads".to_string(),
            check_interval_minutes: 60,
            yt_dlp_path: "/usr/local/bin/yt-dlp".to_string(),
            quality_preset: "720p".to_string(),
            notifications_enabled: true,
            dark_mode: true,
            proxy_url: "http://127.0.0.1:7890".to_string(),
            cookie_file: String::new(),
            max_concurrent_downloads: 1,
        };

        StorageService::save_settings(tmp.path(), &settings)
            .expect("save should succeed");

        let loaded = StorageService::load_settings(tmp.path());
        assert_eq!(loaded.download_dir, settings.download_dir);
        assert_eq!(loaded.check_interval_minutes, settings.check_interval_minutes);
        assert_eq!(loaded.yt_dlp_path, settings.yt_dlp_path);
        assert_eq!(loaded.quality_preset, settings.quality_preset);
        assert_eq!(loaded.notifications_enabled, settings.notifications_enabled);
        assert_eq!(loaded.dark_mode, settings.dark_mode);
        assert_eq!(loaded.proxy_url, settings.proxy_url);
    }

    // ── App state storage tests ────────────────────────────────────

    #[test]
    fn test_app_state_empty_dir_returns_defaults() {
        let tmp = setup_temp_dir();
        let state = StorageService::load_state(tmp.path())
            .expect("should not error on missing file");
        assert_eq!(state.last_check_time, None);
        assert_eq!(state.total_downloads, 0);
    }

    #[test]
    fn test_app_state_save_and_load_roundtrip() {
        let tmp = setup_temp_dir();
        let state = AppState {
            last_check_time: Some("2025-05-28T15:00:00Z".to_string()),
            total_downloads: 123,
        };

        StorageService::save_state(tmp.path(), &state)
            .expect("save should succeed");

        let loaded = StorageService::load_state(tmp.path())
            .expect("load should succeed");
        assert_eq!(loaded.last_check_time, state.last_check_time);
        assert_eq!(loaded.total_downloads, state.total_downloads);
    }

    #[test]
    fn test_write_json_creates_parent_dirs() {
        let tmp = setup_temp_dir();
        let nested = tmp.path().join("deeply").join("nested").join("dir");
        let settings = AppSettings::default();

        // This should auto-create the nested directories
        StorageService::save_settings(&nested, &settings)
            .expect("save should create parent dirs");

        assert!(nested.join("settings.json").exists());
    }

    #[test]
    fn test_file_created_on_save() {
        let tmp = setup_temp_dir();
        let sub_file = tmp.path().join("subscriptions.json");
        assert!(!sub_file.exists());

        StorageService::save_subscriptions(tmp.path(), &[])
            .expect("save should succeed");
        assert!(sub_file.exists(), "file should be created even for empty data");
    }
}
