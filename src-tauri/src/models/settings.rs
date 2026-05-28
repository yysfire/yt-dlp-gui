use serde::{Deserialize, Serialize};

/// Application-wide settings persisted to settings.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Directory where downloaded videos are stored
    pub download_dir: String,
    /// Interval in minutes between automatic subscription checks
    pub check_interval_minutes: u32,
    /// Path to the yt-dlp executable
    pub yt_dlp_path: String,
    /// Default video quality preset
    pub quality_preset: String,
    /// Whether to show desktop notifications on completion
    pub notifications_enabled: bool,
    /// Whether dark mode UI is active
    pub dark_mode: bool,
    /// HTTP/SOCKS proxy URL for yt-dlp (empty string = no proxy)
    pub proxy_url: String,
    /// Path to a Netscape-format cookie file for yt-dlp authentication
    pub cookie_file: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        let download_dir = default_download_dir();
        Self {
            download_dir,
            check_interval_minutes: 360,
            yt_dlp_path: "yt-dlp".to_string(),
            quality_preset: "1080p".to_string(),
            notifications_enabled: false,
            dark_mode: false,
            proxy_url: String::new(),
            cookie_file: String::new(),
        }
    }
}

/// Runtime application state persisted to state.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    /// ISO 8601 timestamp of the last time all subscriptions were checked
    pub last_check_time: Option<String>,
    /// Running total of completed downloads
    pub total_downloads: u32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            last_check_time: None,
            total_downloads: 0,
        }
    }
}

/// Returns a platform-appropriate default download directory.
fn default_download_dir() -> String {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .map(|p| format!("{}\\Videos\\yt-dlp", p))
            .unwrap_or_else(|_| "./downloads".to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .map(|p| format!("{}/Videos/yt-dlp", p))
            .unwrap_or_else(|_| "./downloads".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_settings_default_values() {
        let settings = AppSettings::default();

        // Verify default values
        assert_eq!(settings.check_interval_minutes, 360);
        assert_eq!(settings.yt_dlp_path, "yt-dlp");
        assert_eq!(settings.quality_preset, "1080p");
        assert!(!settings.notifications_enabled);
        assert!(!settings.dark_mode);
        assert_eq!(settings.proxy_url, "");
        assert_eq!(settings.cookie_file, "");
    }

    #[test]
    fn test_app_settings_default_download_dir_not_empty() {
        let settings = AppSettings::default();
        assert!(!settings.download_dir.is_empty(), "download_dir should not be empty");
    }

    #[test]
    fn test_app_settings_serde_roundtrip() {
        let settings = AppSettings::default();
        let json = serde_json::to_string(&settings).expect("serialization should succeed");
        let deserialized: AppSettings =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.download_dir, settings.download_dir);
        assert_eq!(deserialized.check_interval_minutes, settings.check_interval_minutes);
        assert_eq!(deserialized.yt_dlp_path, settings.yt_dlp_path);
        assert_eq!(deserialized.quality_preset, settings.quality_preset);
        assert_eq!(deserialized.notifications_enabled, settings.notifications_enabled);
        assert_eq!(deserialized.dark_mode, settings.dark_mode);
        assert_eq!(deserialized.proxy_url, settings.proxy_url);
        assert_eq!(deserialized.cookie_file, settings.cookie_file);
    }

    #[test]
    fn test_app_settings_json_keys() {
        let settings = AppSettings::default();
        let json_value = serde_json::to_value(&settings).expect("should serialize");

        // Verify all expected keys exist
        assert!(json_value["download_dir"].is_string());
        assert!(json_value["check_interval_minutes"].is_number());
        assert!(json_value["yt_dlp_path"].is_string());
        assert!(json_value["quality_preset"].is_string());
        assert!(json_value["notifications_enabled"].is_boolean());
        assert!(json_value["dark_mode"].is_boolean());
        assert!(json_value["proxy_url"].is_string());
        assert!(json_value["cookie_file"].is_string());
    }

    #[test]
    fn test_app_settings_clone_equality() {
        let s1 = AppSettings::default();
        let s2 = s1.clone();
        assert_eq!(s1.download_dir, s2.download_dir);
        assert_eq!(s1.check_interval_minutes, s2.check_interval_minutes);
        assert_eq!(s1.quality_preset, s2.quality_preset);
    }

    // ── AppState tests ─────────────────────────────────────────────

    #[test]
    fn test_app_state_default_values() {
        let state = AppState::default();

        assert_eq!(state.last_check_time, None);
        assert_eq!(state.total_downloads, 0);
    }

    #[test]
    fn test_app_state_serde_roundtrip() {
        let state = AppState {
            last_check_time: Some("2025-05-28T12:00:00+00:00".to_string()),
            total_downloads: 42,
        };

        let json = serde_json::to_string(&state).expect("serialization should succeed");
        let deserialized: AppState =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.last_check_time, state.last_check_time);
        assert_eq!(deserialized.total_downloads, state.total_downloads);
    }

    #[test]
    fn test_app_state_serde_with_none_last_check() {
        let state = AppState::default();
        let json = serde_json::to_string(&state).expect("serialization should succeed");

        // Should contain null for last_check_time
        assert!(json.contains("\"last_check_time\":null") || json.contains("\"last_check_time\": null"));

        let deserialized: AppState =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(deserialized.last_check_time, None);
    }

    #[test]
    fn test_app_state_json_keys() {
        let state = AppState {
            last_check_time: Some("2025-01-01T00:00:00Z".to_string()),
            total_downloads: 100,
        };
        let json_value = serde_json::to_value(&state).expect("should serialize");

        assert!(json_value["total_downloads"].is_number());
        assert_eq!(json_value["total_downloads"].as_u64().unwrap(), 100);
    }
}
