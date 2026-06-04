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
}

impl DownloadRecord {
    /// Creates a new download record in "downloading" state.
    /// The record is assigned a UUID and the current UTC timestamp.
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
}
