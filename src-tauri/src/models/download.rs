use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a single download record for a video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRecord {
    /// Unique identifier (UUID v4)
    pub id: String,
    /// References the Subscription that triggered this download
    pub subscription_id: String,
    /// Title of the downloaded video
    pub video_title: String,
    /// Original URL of the video
    pub video_url: String,
    /// Local file path where the video was saved
    pub file_path: String,
    /// File size in bytes
    pub file_size: u64,
    /// Download status: "downloading", "completed", "failed", "paused", "cancelled", "waiting", "deleted"
    pub status: String,
    /// ISO 8601 timestamp of download
    pub downloaded_at: String,
    /// Platform-specific video ID used for deduplication
    #[serde(default)]
    pub video_id: String,
    /// Error message if the download failed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// 下载时**请求的画质 preset**（如 "1080p" / "best"）。
    ///
    /// 存请求值而非实际分辨率：若存实际分辨率，用户选 1080p 而源最高只有 720p 时
    /// 会永远误报「可升级」。**空串 = 未知**（旧记录），永不参与升级判定。
    #[serde(default)]
    pub quality: String,
    /// 本轮下载过程中已重试的次数（0..=3）。成功时**保留**，新一轮下载开始时归零。
    #[serde(default)]
    pub retry_count: u32,
    /// 最近一次重试尝试的时间（ISO 8601），未重试过为 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_retry_at: Option<String>,
}

impl DownloadRecord {
    /// Creates a new download record in "downloading" state.
    /// The record is assigned a UUID and the current UTC timestamp.
    ///
    /// 签名保持不变：`quality` 由调用方在构造后按订阅当前 preset 赋值
    /// （生产调用点只有 `download_queue.rs` 的入队路径）。
    pub fn new(subscription_id: String, video_title: String, video_url: String, video_id: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            subscription_id,
            video_title,
            video_url,
            file_path: String::new(),
            file_size: 0,
            status: "downloading".to_string(),
            downloaded_at: Utc::now().to_rfc3339(),
            video_id,
            error_message: None,
            quality: String::new(),
            retry_count: 0,
            last_retry_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_record_new_defaults() {
        let record = DownloadRecord::new(
            "sub-123".to_string(),
            "My Video Title".to_string(),
            "https://youtube.com/watch?v=abc".to_string(),
            "".to_string(),
        );

        // Verify UUID
        assert!(!record.id.is_empty());
        assert_eq!(record.id.len(), 36);
        assert!(uuid::Uuid::parse_str(&record.id).is_ok());

        // Verify constructor args
        assert_eq!(record.subscription_id, "sub-123");
        assert_eq!(record.video_title, "My Video Title");
        assert_eq!(record.video_url, "https://youtube.com/watch?v=abc");

        // Verify defaults
        assert_eq!(record.file_path, "", "file_path should default to empty string");
        assert_eq!(record.file_size, 0, "file_size should default to 0");
        assert_eq!(record.status, "downloading", "status should default to 'downloading'");

        // Verify timestamp
        assert!(!record.downloaded_at.is_empty());
        let parsed = chrono::DateTime::parse_from_rfc3339(&record.downloaded_at);
        assert!(parsed.is_ok(), "downloaded_at should be a valid RFC 3339 timestamp");
    }

    #[test]
    fn test_download_record_status_transitions() {
        let mut record = DownloadRecord::new(
            "sub-1".to_string(),
            "Video".to_string(),
            "https://example.com/v".to_string(),
            "".to_string(),
        );
        assert_eq!(record.status, "downloading");

        // Simulate completed
        record.status = "completed".to_string();
        record.file_path = "/downloads/video.mp4".to_string();
        record.file_size = 123456789;
        record.downloaded_at = Utc::now().to_rfc3339();
        assert_eq!(record.status, "completed");
        assert_eq!(record.file_path, "/downloads/video.mp4");
        assert_eq!(record.file_size, 123456789);

        // Simulate failed
        let mut record2 = DownloadRecord::new(
            "sub-2".to_string(),
            "Failed Video".to_string(),
            "https://example.com/f".to_string(),
            "".to_string(),
        );
        record2.status = "failed".to_string();
        assert_eq!(record2.status, "failed");
        assert_eq!(record2.file_path, ""); // unchanged, download failed
        assert_eq!(record2.file_size, 0);
    }

    #[test]
    fn test_download_record_serde_roundtrip() {
        let record = DownloadRecord {
            id: "test-id".to_string(),
            subscription_id: "sub-id".to_string(),
            video_title: "Test Video".to_string(),
            video_url: "https://example.com/video".to_string(),
            file_path: "/tmp/video.mp4".to_string(),
            file_size: 999_999,
            status: "completed".to_string(),
            downloaded_at: "2025-05-28T12:00:00+00:00".to_string(),
            video_id: "dQw4w9WgXcQ".to_string(),
            error_message: None,
            quality: "1080p".to_string(),
            retry_count: 0,
            last_retry_at: None,
        };

        let json = serde_json::to_string(&record).expect("serialization should succeed");
        let deserialized: DownloadRecord =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.id, record.id);
        assert_eq!(deserialized.subscription_id, record.subscription_id);
        assert_eq!(deserialized.video_title, record.video_title);
        assert_eq!(deserialized.video_url, record.video_url);
        assert_eq!(deserialized.file_path, record.file_path);
        assert_eq!(deserialized.file_size, record.file_size);
        assert_eq!(deserialized.status, record.status);
        assert_eq!(deserialized.downloaded_at, record.downloaded_at);
    }

    #[test]
    fn test_download_record_json_format() {
        let record = DownloadRecord::new(
            "sid".to_string(),
            "Vid".to_string(),
            "https://u".to_string(),
            "".to_string(),
        );

        let json = serde_json::to_value(&record).expect("should serialize to JSON value");

        assert!(json["id"].is_string());
        assert!(json["subscription_id"].is_string());
        assert!(json["video_title"].is_string());
        assert!(json["video_url"].is_string());
        assert!(json["file_path"].is_string());
        assert!(json["file_size"].is_number());
        assert!(json["status"].is_string());
        assert!(json["downloaded_at"].is_string());

        assert_eq!(json["status"].as_str().unwrap(), "downloading");
        assert_eq!(json["file_size"].as_u64().unwrap(), 0);
    }

    #[test]
    fn test_download_record_with_video_id() {
        let record = DownloadRecord::new(
            "sub-1".to_string(),
            "Video".to_string(),
            "https://example.com/v".to_string(),
            "dQw4w9WgXcQ".to_string(),
        );
        assert_eq!(record.video_id, "dQw4w9WgXcQ");
        assert_eq!(record.error_message, None);
    }

    #[test]
    fn test_download_record_error_message_serialization() {
        let mut record = DownloadRecord::new(
            "sub-1".to_string(),
            "Failed Video".to_string(),
            "https://example.com/f".to_string(),
            "abc123".to_string(),
        );
        record.status = "failed".to_string();
        record.error_message = Some("yt-dlp error: Network error".to_string());

        let json = serde_json::to_string(&record).expect("serialization should succeed");
        assert!(json.contains("error_message"));
        assert!(json.contains("Network error"));
    }

    #[test]
    fn test_download_record_error_message_none_skipped() {
        let record = DownloadRecord::new(
            "sub-1".to_string(),
            "Video".to_string(),
            "https://example.com/v".to_string(),
            "xyz".to_string(),
        );
        let json = serde_json::to_string(&record).expect("serialization should succeed");
        assert!(!json.contains("error_message") || json.contains("\"error_message\":null"));
    }

    #[test]
    fn test_download_record_status_paused_and_cancelled() {
        let mut record = DownloadRecord::new(
            "sub-1".to_string(),
            "Video".to_string(),
            "https://example.com/v".to_string(),
            "abc".to_string(),
        );

        record.status = "paused".to_string();
        assert_eq!(record.status, "paused");

        record.status = "cancelled".to_string();
        assert_eq!(record.status, "cancelled");
    }

    // ── Rust ↔ TS 契约基准（共享 fixture）──────────────────────────
    //
    // fixture 路径：`src-tauri/tests/fixtures/download_record.json`，前端契约测试
    // （`src/types/__tests__/downloadRecordContract.test.ts`）读**同一个文件**。
    // 改 fixture ⇒ 两侧必有一侧变红。
    //
    // 注意 fixture 里**没有** `error_message`：该字段带
    // `skip_serializing_if = "Option::is_none"`，而本基准记录是 `completed`
    // （成功时清空 error_message）⇒ 序列化时该键缺省。因此 fixture 必须与
    // `to_value` 的输出**逐键**一致，不能写 `"error_message": null`。

    const DOWNLOAD_RECORD_FIXTURE: &str =
        include_str!("../../tests/fixtures/download_record.json");

    /// 基准记录：除 `error_message`（None）外字段填满。`status = completed` 与
    /// `retry_count = 2` 是**有意**组合 —— 锁住「成功时保留重试次数」。
    fn contract_baseline() -> DownloadRecord {
        DownloadRecord {
            id: "0f0a9c1e-6b0a-4a7c-9f1b-2d3e4f5a6b7c".to_string(),
            subscription_id: "1a2b3c4d-5e6f-4a8b-9c0d-1e2f3a4b5c6d".to_string(),
            video_id: "dQw4w9WgXcQ".to_string(),
            video_title: "示例视频".to_string(),
            video_url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
            file_path: "/home/user/Videos/yt-dlp/示例视频.mp4".to_string(),
            file_size: 123_456_789,
            status: "completed".to_string(),
            downloaded_at: "2026-10-02T12:34:56.789+00:00".to_string(),
            error_message: None,
            quality: "1080p".to_string(),
            retry_count: 2,
            last_retry_at: Some("2026-10-02T12:30:00+00:00".to_string()),
        }
    }

    #[test]
    fn test_download_record_matches_shared_fixture() {
        let fixture: serde_json::Value =
            serde_json::from_str(DOWNLOAD_RECORD_FIXTURE).expect("fixture 必须是合法 JSON");
        let serialized =
            serde_json::to_value(contract_baseline()).expect("基准记录必须能序列化");
        assert_eq!(
            serialized, fixture,
            "Rust 侧序列化输出与共享 fixture 不一致 —— 改字段时两侧都要动"
        );
    }

    #[test]
    fn test_download_record_none_error_message_key_absent() {
        // `error_message = None` 时该键必须**不出现**，而不是 `null`
        let record = contract_baseline();
        assert_eq!(record.error_message, None);
        let serialized = serde_json::to_value(&record).expect("序列化");
        assert!(
            serialized.get("error_message").is_none(),
            "error_message 为 None 时不应出现该键，实际: {}",
            serialized
        );
    }

    #[test]
    fn test_download_record_old_json_uses_new_field_defaults() {
        // 旧 download_records.json 没有三个新字段：必须能加载并取默认值
        let old_json = r#"{
            "id": "old-1",
            "subscription_id": "sub-1",
            "video_title": "旧视频",
            "video_url": "https://example.com/old",
            "file_path": "/tmp/old.mp4",
            "file_size": 100,
            "status": "completed",
            "downloaded_at": "2025-01-01T00:00:00Z"
        }"#;

        let record: DownloadRecord =
            serde_json::from_str(old_json).expect("旧 JSON 必须能反序列化");

        assert_eq!(record.video_id, "", "缺失的 video_id 取默认空串");
        assert_eq!(record.quality, "", "缺失的 quality 取默认空串（= 未知）");
        assert_eq!(record.retry_count, 0, "缺失的 retry_count 取默认 0");
        assert_eq!(record.last_retry_at, None, "缺失的 last_retry_at 取默认 None");
    }
}
