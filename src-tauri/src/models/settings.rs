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
    /// Maximum number of concurrent downloads (1-5)
    #[serde(default = "default_max_concurrent_downloads")]
    pub max_concurrent_downloads: u32,
    /// When true, window minimizes to system tray instead of taskbar
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    /// When true, closing the window hides to tray instead of exiting
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    /// When true, application starts minimized to system tray
    #[serde(default)]
    pub start_in_tray: bool,
    /// When true, the background scheduler is paused (tray menu toggle)
    #[serde(default)]
    pub scheduler_paused: bool,
}

fn default_max_concurrent_downloads() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        let download_dir = default_download_dir();
        Self {
            download_dir,
            check_interval_minutes: 60,
            yt_dlp_path: "yt-dlp".to_string(),
            quality_preset: "1080p".to_string(),
            notifications_enabled: false,
            dark_mode: false,
            proxy_url: String::new(),
            cookie_file: String::new(),
            max_concurrent_downloads: 1,
            minimize_to_tray: true,
            close_to_tray: true,
            start_in_tray: false,
            scheduler_paused: false,
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
#[doc(hidden)]
pub fn default_download_dir() -> String {
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
        assert_eq!(settings.check_interval_minutes, 60);
        assert_eq!(settings.yt_dlp_path, "yt-dlp");
        assert_eq!(settings.quality_preset, "1080p");
        assert!(!settings.notifications_enabled);
        assert!(!settings.dark_mode);
        assert_eq!(settings.proxy_url, "");
        assert_eq!(settings.cookie_file, "");
        assert!(settings.minimize_to_tray);
        assert!(settings.close_to_tray);
        assert!(!settings.start_in_tray);
        assert!(!settings.scheduler_paused);
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
        assert!(json_value["max_concurrent_downloads"].is_number());
    }

    #[test]
    fn test_max_concurrent_downloads_default() {
        let settings = AppSettings::default();
        assert_eq!(settings.max_concurrent_downloads, 1);
    }

    #[test]
    fn test_max_concurrent_downloads_custom_value() {
        let json = r#"{
            "download_dir": "/tmp",
            "check_interval_minutes": 60,
            "yt_dlp_path": "yt-dlp",
            "quality_preset": "1080p",
            "notifications_enabled": true,
            "dark_mode": false,
            "proxy_url": "",
            "cookie_file": "",
            "max_concurrent_downloads": 5
        }"#;
        let settings: AppSettings = serde_json::from_str(json)
            .expect("should deserialize with max_concurrent_downloads");
        assert_eq!(settings.max_concurrent_downloads, 5);
    }

    #[test]
    fn test_max_concurrent_downloads_default_on_missing() {
        let json = r#"{
            "download_dir": "/tmp",
            "check_interval_minutes": 60,
            "yt_dlp_path": "yt-dlp",
            "quality_preset": "1080p",
            "notifications_enabled": false,
            "dark_mode": false,
            "proxy_url": "",
            "cookie_file": ""
        }"#;
        let settings: AppSettings = serde_json::from_str(json)
            .expect("should deserialize with missing max_concurrent_downloads");
        assert_eq!(settings.max_concurrent_downloads, 1);
    }

    #[test]
    fn test_app_settings_clone_equality() {
        let s1 = AppSettings::default();
        let s2 = s1.clone();
        assert_eq!(s1.download_dir, s2.download_dir);
        assert_eq!(s1.check_interval_minutes, s2.check_interval_minutes);
        assert_eq!(s1.quality_preset, s2.quality_preset);
    }

    #[test]
    fn test_corrupt_settings_returns_defaults() {
        // T018: FR-009, SC-005 — corrupt JSON must fall back to defaults
        let corrupt_json = "{ not valid json at all !! }";
        let result: Result<AppSettings, _> = serde_json::from_str(corrupt_json);
        assert!(
            result.is_err(),
            "Corrupt JSON should fail deserialization"
        );
    }

    // ── Tray settings tests (spec 005-system-tray-icon) ──────────────

    #[test]
    fn test_tray_settings_defaults() {
        let settings = AppSettings::default();
        assert!(settings.minimize_to_tray, "minimize_to_tray should default to true");
        assert!(settings.close_to_tray, "close_to_tray should default to true");
        assert!(!settings.start_in_tray, "start_in_tray should default to false");
        assert!(!settings.scheduler_paused, "scheduler_paused should default to false");
    }

    #[test]
    fn test_tray_settings_serde_roundtrip() {
        let settings = AppSettings {
            minimize_to_tray: false,
            close_to_tray: false,
            start_in_tray: true,
            scheduler_paused: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).expect("serialize");
        let deserialized: AppSettings = serde_json::from_str(&json).expect("deserialize");
        assert!(!deserialized.minimize_to_tray);
        assert!(!deserialized.close_to_tray);
        assert!(deserialized.start_in_tray);
        assert!(deserialized.scheduler_paused);
    }

    #[test]
    fn test_tray_settings_default_on_missing() {
        let json = r#"{
            "download_dir": "/tmp",
            "check_interval_minutes": 60,
            "yt_dlp_path": "yt-dlp",
            "quality_preset": "1080p",
            "notifications_enabled": false,
            "dark_mode": false,
            "proxy_url": "",
            "cookie_file": ""
        }"#;
        let settings: AppSettings = serde_json::from_str(json)
            .expect("should deserialize with missing tray fields");
        // Tray fields should get their serde defaults when missing
        assert!(settings.minimize_to_tray);
        assert!(settings.close_to_tray);
        assert!(!settings.start_in_tray);
        assert!(!settings.scheduler_paused);
    }

    #[test]
    fn test_tray_settings_json_keys() {
        let settings = AppSettings::default();
        let json_value = serde_json::to_value(&settings).expect("should serialize");
        assert!(json_value["minimize_to_tray"].is_boolean());
        assert!(json_value["close_to_tray"].is_boolean());
        assert!(json_value["start_in_tray"].is_boolean());
        assert!(json_value["scheduler_paused"].is_boolean());
    }

    #[test]
    fn test_settings_persist_across_reload() {
        // T042: SC-003 — round-trip through serialization simulates persistence
        let original = AppSettings {
            download_dir: "/custom/path".to_string(),
            check_interval_minutes: 30,
            quality_preset: "720p".to_string(),
            proxy_url: "http://127.0.0.1:8080".to_string(),
            max_concurrent_downloads: 3,
            ..Default::default()
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let reloaded: AppSettings = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(reloaded.download_dir, original.download_dir);
        assert_eq!(reloaded.check_interval_minutes, original.check_interval_minutes);
        assert_eq!(reloaded.quality_preset, original.quality_preset);
        assert_eq!(reloaded.proxy_url, original.proxy_url);
        assert_eq!(reloaded.max_concurrent_downloads, original.max_concurrent_downloads);
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
