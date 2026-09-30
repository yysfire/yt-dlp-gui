use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::models::{AppSettings, AppState, DownloadRecord, Subscription};
use crate::utils::AppError;

/// Stateless service for reading/writing JSON persistence files.
pub struct StorageService;

/// 全局写事务锁。
///
/// 所有写路径（`update_*` 与 `save_settings`）都必须经过它，使「读当前内容 → 改内存
/// → 写回」成为原子操作，消除并发读改写（RMW）造成的丢失更新。
///
/// 读路径（`load_*`）不加锁：`write_json` 采用「写同目录临时文件 + 原子 rename」，
/// 读端不会看到被截断的半个文件。
///
/// 锁序：应用里唯一涉及本锁的嵌套是 `QueueContext.queue → 本锁`（`commands/download.rs`
/// 持队列锁调用 `enqueue_from_video`，后者先完成 storage 写、之后才锁自身队列）。
/// 维持「无反向路径」结论的前提是事务闭包不获取任何其它锁 —— 见 `update_*` 的约束说明。
static WRITE_LOCK: Mutex<()> = Mutex::new(());

// debug 构建下标记「本线程正处于写事务中」，用于捕获会导致死锁的嵌套调用。
#[cfg(debug_assertions)]
thread_local! {
    static IN_TXN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 退栈时复位 `IN_TXN`，闭包 panic 时同样生效。
#[cfg(debug_assertions)]
struct TxnFlag;

#[cfg(debug_assertions)]
impl Drop for TxnFlag {
    fn drop(&mut self) {
        IN_TXN.with(|c| c.set(false));
    }
}

/// 在全局写锁保护下执行 `f`。
///
/// 锁被 poison 时恢复而非报错：被守护的数据是 `()`，真实状态都在磁盘上，
/// 让一次 panic 永久瘫痪所有持久化写入并不划算。
fn with_write_lock<R>(f: impl FnOnce() -> Result<R, AppError>) -> Result<R, AppError> {
    // 必须在 lock() 之前检查：同线程重入会在 lock() 处直接死锁，根本到不了检查。
    #[cfg(debug_assertions)]
    {
        if IN_TXN.with(|c| c.get()) {
            panic!("storage: 检测到嵌套写事务（update_*/save_settings 重入），会死锁");
        }
        IN_TXN.with(|c| c.set(true));
    }
    #[cfg(debug_assertions)]
    let _flag = TxnFlag;

    let _guard = WRITE_LOCK.lock().unwrap_or_else(|poisoned| {
        log::warn!("storage: 写锁已被 poison，已恢复");
        poisoned.into_inner()
    });

    f()
}

impl StorageService {
    // ── 事务式写入 ─────────────────────────────────────────────────
    //
    // 以下 update_* 是生产代码唯一的写入接口。闭包约束（务必遵守）：
    //
    // 1. 禁止调用 `update_*` 或 `save_settings`：`std::sync::Mutex` 不可重入，会死锁
    //    （debug 构建下会 panic 提示）。
    // 2. 禁止获取任何应用级锁（`AppContext.settings`、`QueueContext.queue`、
    //    `DownloadQueue` 的队列/任务锁、`TrayService.state`、`ImportContext.cancel_flag`）：
    //    会与「持队列锁 → 本锁」的顺序形成反向环。
    // 3. 禁止做文件 I/O：会长时间占用写锁，阻塞下载队列的记录更新。
    // 4. 如需读取其它实体，请在闭包外先读好再传入。闭包内调 `load_*` 不会死锁，
    //    但读到的是本事务开始之前的快照。

    /// 以事务方式修改 `download_records.json`。
    ///
    /// 拿写锁 → 读取磁盘当前内容 → 交给 `f` → `f` 返回 `Ok` 时写回并返回其值；
    /// 返回 `Err` 时不写回（丢弃内存改动）并把错误原样返回。
    pub fn update_download_records<R>(
        data_dir: &Path,
        f: impl FnOnce(&mut Vec<DownloadRecord>) -> Result<R, AppError>,
    ) -> Result<R, AppError> {
        let path = data_dir.join("download_records.json");
        with_write_lock(|| {
            let mut records: Vec<DownloadRecord> = Self::read_json_array(&path)?;
            let result = f(&mut records)?;
            Self::save_download_records(data_dir, &records)?;
            Ok(result)
        })
    }

    /// 以事务方式修改 `subscriptions.json`。语义与约束同 [`Self::update_download_records`]。
    pub fn update_subscriptions<R>(
        data_dir: &Path,
        f: impl FnOnce(&mut Vec<Subscription>) -> Result<R, AppError>,
    ) -> Result<R, AppError> {
        let path = data_dir.join("subscriptions.json");
        with_write_lock(|| {
            let mut subs: Vec<Subscription> = Self::read_json_array(&path)?;
            let result = f(&mut subs)?;
            Self::save_subscriptions(data_dir, &subs)?;
            Ok(result)
        })
    }

    /// 以事务方式修改 `state.json`（单对象，不是数组）。语义与约束同
    /// [`Self::update_download_records`]；文件缺失或损坏时以默认值起算。
    pub fn update_state<R>(
        data_dir: &Path,
        f: impl FnOnce(&mut AppState) -> Result<R, AppError>,
    ) -> Result<R, AppError> {
        let path = data_dir.join("state.json");
        with_write_lock(|| {
            let mut state: AppState = Self::read_json(&path).unwrap_or_default();
            let result = f(&mut state)?;
            Self::save_state(data_dir, &state)?;
            Ok(result)
        })
    }

    // ── Subscriptions ──────────────────────────────────────────────

    /// Loads all subscriptions from `subscriptions.json`.
    /// Returns an empty Vec if the file does not exist.
    pub fn load_subscriptions(data_dir: &Path) -> Result<Vec<Subscription>, AppError> {
        let path = data_dir.join("subscriptions.json");
        Self::read_json_array(&path)
    }

    /// 将完整订阅列表写入 `subscriptions.json`。
    ///
    /// 私有：生产代码一律走 [`Self::update_subscriptions`] 事务，以便由编译器
    /// 保证不存在绕过写锁的读改写。
    fn save_subscriptions(
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

    /// 将完整下载记录列表写入 `download_records.json`。
    ///
    /// 私有：生产代码一律走 [`Self::update_download_records`] 事务。
    fn save_download_records(
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
        Self::update_download_records(data_dir, |records| {
            let original_count = records.len();
            let deduped = Self::deduplicate_vec(std::mem::take(records));
            let removed = original_count - deduped.len();
            if removed > 0 {
                log::info!(
                    "Deduplicated download records: removed {} duplicate(s), {} → {} records",
                    removed,
                    original_count,
                    deduped.len()
                );
            }
            *records = deduped;
            Ok(removed)
        })
    }

    // ── Settings ────────────────────────────────────────────────────

    /// Loads settings from `settings.json`. Returns defaults if the file
    /// does not exist or is corrupt.
    pub fn load_settings(data_dir: &Path) -> AppSettings {
        let path = data_dir.join("settings.json");
        Self::read_json(&path).unwrap_or_default()
    }

    /// Saves settings to `settings.json`.
    ///
    /// 走全局写锁：settings 是整对象替换，但仍需与并发写者（如托盘菜单切换）
    /// 串行，且避免两次并发写落到同一个 `settings.tmp`。
    pub fn save_settings(data_dir: &Path, settings: &AppSettings) -> Result<(), AppError> {
        let path = data_dir.join("settings.json");
        with_write_lock(|| Self::write_json(&path, settings))
    }

    // ── App State ───────────────────────────────────────────────────

    /// Loads application state from `state.json`. Returns defaults if absent.
    pub fn load_state(data_dir: &Path) -> Result<AppState, AppError> {
        let path = data_dir.join("state.json");
        Ok(Self::read_json(&path).unwrap_or_default())
    }

    /// 将应用状态写入 `state.json`。
    ///
    /// 私有：生产代码一律走 [`Self::update_state`] 事务。
    fn save_state(data_dir: &Path, state: &AppState) -> Result<(), AppError> {
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
    ///
    /// 采用「写同目录临时文件 + 原子 rename」而非直接覆写：直接 `std::fs::write`
    /// 会原地截断，并发读（`load_*`，不加锁）可能读到半个文件而反序列化失败。
    /// 原子替换后读端只可能看到「旧完整版」或「新完整版」。
    fn write_json<T: serde::Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), AppError> {
        let json = serde_json::to_string_pretty(value)?;
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, json)?;

        // Windows 上若目标文件正被外部进程以不共享删除的方式打开（杀软、索引器、
        // 资源管理器预览等），rename 会返回 ERROR_ACCESS_DENIED。做有限退避重试。
        let mut last_err = None;
        for attempt in 0..3u8 {
            match std::fs::rename(&tmp, path) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 2 {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            }
        }

        let _ = std::fs::remove_file(&tmp);
        Err(last_err
            .expect("rename 至少尝试过一次")
            .into())
    }
}

#[cfg(test)]
impl StorageService {
    /// 测试专用：整体覆盖订阅集合（`save_subscriptions` 私有化后的播种入口）。
    pub(crate) fn seed_subscriptions(
        data_dir: &Path,
        subs: &[Subscription],
    ) -> Result<(), AppError> {
        Self::update_subscriptions(data_dir, |current| {
            *current = subs.to_vec();
            Ok(())
        })
    }

    /// 测试专用：整体覆盖下载记录集合（`save_download_records` 私有化后的播种入口）。
    pub(crate) fn seed_download_records(
        data_dir: &Path,
        records: &[DownloadRecord],
    ) -> Result<(), AppError> {
        Self::update_download_records(data_dir, |current| {
            *current = records.to_vec();
            Ok(())
        })
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
            ..Default::default()
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
    }

    #[test]
    fn test_app_state_save_and_load_roundtrip() {
        let tmp = setup_temp_dir();
        let state = AppState {
            last_check_time: Some("2025-05-28T15:00:00Z".to_string()),
        };

        StorageService::save_state(tmp.path(), &state)
            .expect("save should succeed");

        let loaded = StorageService::load_state(tmp.path())
            .expect("load should succeed");
        assert_eq!(loaded.last_check_time, state.last_check_time);
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

    // ── 事务接口不变量测试 ──────────────────────────────────────────

    #[test]
    fn test_update_transaction_commits_and_returns_value() {
        let tmp = setup_temp_dir();
        let dir = tmp.path();
        let record = make_record("sub-1", "Video");

        let returned = StorageService::update_download_records(dir, |records| {
            records.push(record.clone());
            Ok(records.len())
        })
        .expect("update should succeed");

        assert_eq!(returned, 1, "应返回闭包的返回值");
        let records = StorageService::load_download_records(dir).expect("load should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, record.id);
    }

    #[test]
    fn test_update_transaction_rolls_back_on_error() {
        let tmp = setup_temp_dir();
        let dir = tmp.path();
        let original = make_record("sub-1", "Original");
        StorageService::seed_download_records(dir, &[original.clone()]).expect("seed failed");

        let result: Result<(), AppError> = StorageService::update_download_records(dir, |records| {
            records.push(make_record("sub-1", "ShouldNotPersist"));
            Err(AppError::NotFound("模拟失败".to_string()))
        });

        assert!(result.is_err(), "闭包返回 Err 时事务应失败");
        let records = StorageService::load_download_records(dir).expect("load should succeed");
        assert_eq!(records.len(), 1, "闭包 Err 时不应写回内存改动");
        assert_eq!(records[0].id, original.id);
    }

    /// 直击原始 bug：并发读改写（load → mutate → save）会丢失更新。
    #[test]
    fn test_update_concurrent_writes_do_not_lose_updates() {
        let tmp = setup_temp_dir();
        let dir = tmp.path();
        const N: usize = 16;

        std::thread::scope(|scope| {
            for i in 0..N {
                scope.spawn(move || {
                    let mut record = make_record("sub-1", &format!("Video {}", i));
                    record.video_id = format!("vid-{}", i);
                    StorageService::update_download_records(dir, |records| {
                        records.push(record);
                        Ok(())
                    })
                    .expect("concurrent update should succeed");
                });
            }
        });

        let records = StorageService::load_download_records(dir).expect("load should succeed");
        assert_eq!(records.len(), N, "并发追加不应丢失任何一条记录");
    }

    /// 旧格式 state.json（含已删除的 total_downloads 键）应能被事务更新正常处理，
    /// 且更新后不再写出该键。覆盖「向后兼容」与「事务语义」两点。
    #[test]
    fn test_update_state_handles_legacy_state_file() {
        let tmp = setup_temp_dir();
        let dir = tmp.path();
        let path = dir.join("state.json");
        std::fs::write(
            &path,
            r#"{"last_check_time":"2025-01-01T00:00:00Z","total_downloads":47}"#,
        )
        .expect("seed legacy state file");

        StorageService::update_state(dir, |state| {
            state.last_check_time = Some("2026-01-01T00:00:00Z".to_string());
            Ok(())
        })
        .expect("update should succeed on legacy file");

        let state = StorageService::load_state(dir).expect("load should succeed");
        assert_eq!(state.last_check_time.as_deref(), Some("2026-01-01T00:00:00Z"));

        let raw = std::fs::read_to_string(&path).expect("read state.json");
        assert!(
            !raw.contains("total_downloads"),
            "已删除的字段不应再被写出，实际内容: {}",
            raw
        );
    }

    #[test]
    fn test_atomic_write_leaves_no_tmp_file() {
        let tmp = setup_temp_dir();
        let dir = tmp.path();

        StorageService::update_download_records(dir, |records| {
            records.push(make_record("sub-1", "Video"));
            Ok(())
        })
        .expect("update should succeed");

        assert!(dir.join("download_records.json").exists());
        assert!(
            !dir.join("download_records.tmp").exists(),
            "临时文件应已被 rename 消耗，不应残留"
        );
    }

    /// 事务内再开事务会死锁；debug 构建下应直接 panic 提示。
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "嵌套写事务")]
    fn test_nested_write_transaction_panics_in_debug() {
        let tmp = setup_temp_dir();
        let dir = tmp.path();

        let _ = StorageService::update_state(dir, |_| {
            StorageService::update_subscriptions(dir, |_| Ok(()))
        });
    }
}
