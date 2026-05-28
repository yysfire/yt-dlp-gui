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
    /// ISO 8601 creation timestamp
    pub created_at: String,
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
            created_at: Utc::now().to_rfc3339(),
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
        assert_eq!(deserialized.created_at, sub.created_at);
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
}
