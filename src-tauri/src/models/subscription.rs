use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a channel subscription with its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    /// Unique identifier (UUID v4)
    pub id: String,
    /// Channel or playlist URL
    pub url: String,
    /// Platform identifier: "youtube", "bilibili", "other"
    pub platform: String,
    /// Display name of the channel
    pub channel_name: String,
    /// URL to the channel's avatar/thumbnail
    pub channel_avatar_url: String,
    /// Whether the subscription is paused (skipped during checks)
    pub paused: bool,
    /// Quality preset for downloads (e.g., "1080p", "720p", "best")
    pub quality_preset: String,
    /// Group/category name for organizing subscriptions
    pub group_name: String,
    /// ISO 8601 creation timestamp
    pub created_at: String,
    /// ISO 8601 timestamp of the last check, or None if never checked
    pub last_checked_at: Option<String>,
    /// Number of videos successfully downloaded from this subscription
    #[serde(default)]
    pub download_count: u32,
    /// Status of the last check: "success" or "failed"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check_status: Option<String>,
    /// Error message from the last failed check
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check_error: Option<String>,
    /// Group tags for organizing subscriptions (e.g., ["学习", "音乐"])
    #[serde(default)]
    pub tags: Vec<String>,
    /// Health check result: "ok", "warning", "dead", or null if never checked
    #[serde(default)]
    pub health_status: Option<crate::models::health::HealthStatus>,
    /// Timestamp of the last health check (ISO 8601), or null if never checked
    #[serde(default)]
    pub last_health_check: Option<String>,
}

impl Subscription {
    /// Creates a new subscription with generated UUID and current timestamp.
    /// Default quality preset is "1080p" and paused is false.
    pub fn new(
        url: String,
        platform: String,
        channel_name: String,
        channel_avatar_url: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            url,
            platform,
            channel_name,
            channel_avatar_url,
            paused: false,
            quality_preset: "1080p".to_string(),
            group_name: "未分组".to_string(),
            created_at: Utc::now().to_rfc3339(),
            last_checked_at: None,
            download_count: 0,
            last_check_status: None,
            last_check_error: None,
            tags: Vec::new(),
            health_status: None,
            last_health_check: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_new_fields() {
        let sub = Subscription::new(
            "https://youtube.com/@test".to_string(),
            "youtube".to_string(),
            "Test Channel".to_string(),
            "https://example.com/avatar.jpg".to_string(),
        );

        // Verify UUID is valid v4 format
        assert!(!sub.id.is_empty());
        assert_eq!(sub.id.len(), 36); // UUID v4 string length
        assert!(uuid::Uuid::parse_str(&sub.id).is_ok());

        // Verify fields passed to constructor
        assert_eq!(sub.url, "https://youtube.com/@test");
        assert_eq!(sub.platform, "youtube");
        assert_eq!(sub.channel_name, "Test Channel");
        assert_eq!(sub.channel_avatar_url, "https://example.com/avatar.jpg");

        // Verify defaults
        assert!(!sub.paused, "paused should default to false");
        assert_eq!(sub.quality_preset, "1080p", "quality_preset should default to '1080p'");
        assert_eq!(sub.group_name, "未分组", "group_name should default to '未分组'");
        assert!(sub.last_checked_at.is_none(), "last_checked_at should default to None");

        // Verify created_at is a valid ISO 8601 timestamp
        assert!(!sub.created_at.is_empty());
        let parsed = chrono::DateTime::parse_from_rfc3339(&sub.created_at);
        assert!(parsed.is_ok(), "created_at should be a valid RFC 3339 timestamp");
    }

    #[test]
    fn test_subscription_new_unique_ids() {
        let sub1 = Subscription::new(
            "https://youtube.com/@a".to_string(),
            "youtube".to_string(),
            "A".to_string(),
            "".to_string(),
        );
        let sub2 = Subscription::new(
            "https://youtube.com/@b".to_string(),
            "youtube".to_string(),
            "B".to_string(),
            "".to_string(),
        );

        assert_ne!(sub1.id, sub2.id, "each subscription should have a unique ID");
        assert_ne!(sub1.created_at, sub2.created_at,
            "timestamps should differ for sequentially-created subs (unless clock resolution is low)");
    }

    #[test]
    fn test_subscription_serde_roundtrip() {
        let sub = Subscription::new(
            "https://bilibili.com/space/123".to_string(),
            "bilibili".to_string(),
            "B站频道".to_string(),
            "https://example.com/avatar.png".to_string(),
        );

        let json = serde_json::to_string(&sub).expect("serialization should succeed");
        let deserialized: Subscription =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.id, sub.id);
        assert_eq!(deserialized.url, sub.url);
        assert_eq!(deserialized.platform, sub.platform);
        assert_eq!(deserialized.channel_name, sub.channel_name);
        assert_eq!(deserialized.channel_avatar_url, sub.channel_avatar_url);
        assert_eq!(deserialized.paused, sub.paused);
        assert_eq!(deserialized.quality_preset, sub.quality_preset);
        assert_eq!(deserialized.group_name, sub.group_name);
        assert_eq!(deserialized.created_at, sub.created_at);
        assert_eq!(deserialized.last_checked_at, sub.last_checked_at);
    }

    #[test]
    fn test_subscription_different_platforms() {
        for platform in &["youtube", "bilibili", "other"] {
            let sub = Subscription::new(
                "https://example.com".to_string(),
                platform.to_string(),
                "Test".to_string(),
                "".to_string(),
            );
            assert_eq!(sub.platform, *platform);
        }
    }

    #[test]
    fn test_subscription_default_group_name() {
        let sub = Subscription::new(
            "https://youtube.com/@test".to_string(),
            "youtube".to_string(),
            "Test Channel".to_string(),
            "https://example.com/avatar.jpg".to_string(),
        );
        assert_eq!(sub.group_name, "未分组");
    }

    // ── Per-subscription tracking fields (TDD) ─────────────────────

    #[test]
    fn test_subscription_new_download_count_default() {
        let sub = Subscription::new(
            "https://youtube.com/@test".to_string(),
            "youtube".to_string(),
            "Test".to_string(),
            "".to_string(),
        );
        assert_eq!(sub.download_count, 0);
    }

    #[test]
    fn test_subscription_new_check_status_default() {
        let sub = Subscription::new(
            "https://youtube.com/@test".to_string(),
            "youtube".to_string(),
            "Test".to_string(),
            "".to_string(),
        );
        assert_eq!(sub.last_check_status, None);
        assert_eq!(sub.last_check_error, None);
    }

    #[test]
    fn test_subscription_new_fields_serde_roundtrip() {
        let mut sub = Subscription::new(
            "https://youtube.com/@test".to_string(),
            "youtube".to_string(),
            "Test Channel".to_string(),
            "https://example.com/avatar.jpg".to_string(),
        );
        sub.download_count = 5;
        sub.last_checked_at = Some("2026-06-02T12:00:00Z".to_string());
        sub.last_check_status = Some("success".to_string());
        sub.last_check_error = None;

        let json = serde_json::to_string(&sub).expect("serialization should succeed");
        assert!(json.contains("\"download_count\":5"));
        assert!(json.contains("\"last_checked_at\":\"2026-06-02T12:00:00Z\""));
        assert!(json.contains("\"last_check_status\":\"success\""));

        let deserialized: Subscription =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(deserialized.download_count, 5);
        assert_eq!(deserialized.last_check_status, Some("success".to_string()));
        assert_eq!(deserialized.last_check_error, None);
    }

    #[test]
    fn test_subscription_new_fields_error_state() {
        let mut sub = Subscription::new(
            "https://youtube.com/@test".to_string(),
            "youtube".to_string(),
            "Test Channel".to_string(),
            "".to_string(),
        );
        sub.last_check_status = Some("failed".to_string());
        sub.last_check_error = Some("Network error".to_string());

        let json = serde_json::to_string(&sub).expect("serialization should succeed");
        assert!(json.contains("\"failed\""));
        assert!(json.contains("\"Network error\""));

        let deserialized: Subscription =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(deserialized.last_check_status, Some("failed".to_string()));
        assert_eq!(deserialized.last_check_error, Some("Network error".to_string()));
    }
}
