use serde::{Deserialize, Serialize};

use crate::models::Subscription;

/// Import source type for batch import operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImportSource {
    #[serde(rename = "opml")]
    OpmlFile { path: String },
    #[serde(rename = "txt")]
    TxtFile { path: String },
    #[serde(rename = "url_list")]
    UrlList { urls: Vec<String> },
}

/// Single item in an import preview list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewItem {
    /// The URL extracted from the import source.
    pub url: String,
    /// Channel title from OPML, or null for TXT/URL lists.
    pub title: Option<String>,
    /// Whether this URL already exists in subscriptions.
    pub is_duplicate: bool,
    /// Validation error, or null if the URL is valid.
    pub error: Option<String>,
}

/// Result of `batch_import_preview` — parsed URLs before execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    /// All parsed items from the import source.
    pub items: Vec<ImportPreviewItem>,
    /// Total number of items found in source.
    pub total: usize,
    /// Number of items that duplicate existing subscriptions.
    pub duplicates: usize,
}

/// Progress event payload for `import-progress`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgressEvent {
    /// Unique task identifier.
    pub task_id: String,
    /// Number of URLs processed so far.
    pub completed: usize,
    /// Total URLs to process.
    pub total: usize,
    /// URL currently being processed.
    pub current_url: String,
    /// Title of the channel currently being processed.
    pub current_title: String,
}

/// Completion event payload for `import-complete`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCompleteEvent {
    /// Unique task identifier.
    pub task_id: String,
    /// Total URLs in the import task.
    pub total: usize,
    /// Number of successfully imported subscriptions.
    pub success: usize,
    /// Number of URLs that failed to import.
    pub failed: usize,
    /// Number of URLs skipped (duplicates or invalid).
    pub skipped: usize,
    /// Detailed error information for failed items.
    pub errors: Vec<ImportErrorItem>,
}

/// Individual error item for import failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportErrorItem {
    /// The URL that failed.
    pub url: String,
    /// Reason for the failure.
    pub reason: String,
}

/// Result of a batch import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    /// Subscriptions that were successfully imported.
    pub imported: Vec<Subscription>,
    /// URLs that were skipped because they already exist.
    pub skipped_duplicates: Vec<String>,
    /// URLs that were skipped because parsing/download failed.
    pub skipped_invalid: Vec<String>,
    /// Total number of URLs submitted for import.
    pub total: usize,
    /// Number of successfully imported subscriptions.
    pub success_count: usize,
}

/// Represents an `<outline>` element parsed from an OPML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpmlOutline {
    /// Channel title (from `title` or `text` attribute).
    pub title: String,
    /// Feed/channel URL (from `xmlUrl` attribute).
    pub xml_url: String,
    /// Optional description.
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_result_default_equivalent() {
        // ImportResult doesn't implement Default, but we can construct an empty one
        let result = ImportResult {
            imported: vec![],
            skipped_duplicates: vec![],
            skipped_invalid: vec![],
            total: 0,
            success_count: 0,
        };
        assert_eq!(result.imported.len(), 0);
        assert_eq!(result.skipped_duplicates.len(), 0);
        assert_eq!(result.skipped_invalid.len(), 0);
        assert_eq!(result.total, 0);
        assert_eq!(result.success_count, 0);
    }

    #[test]
    fn test_import_result_serde_roundtrip() {
        let sub = crate::models::Subscription::new(
            "https://youtube.com/@test".to_string(),
            "youtube".to_string(),
            "Test Channel".to_string(),
            "https://avatar.url".to_string(),
        );

        let result = ImportResult {
            imported: vec![sub.clone()],
            skipped_duplicates: vec!["https://dup.example.com".to_string()],
            skipped_invalid: vec!["not-a-url".to_string()],
            total: 3,
            success_count: 1,
        };

        let json = serde_json::to_string(&result).expect("serialization should succeed");
        let deserialized: ImportResult =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.total, 3);
        assert_eq!(deserialized.success_count, 1);
        assert_eq!(deserialized.imported.len(), 1);
        assert_eq!(deserialized.imported[0].url, "https://youtube.com/@test");
        assert_eq!(deserialized.imported[0].channel_name, "Test Channel");
        assert_eq!(deserialized.skipped_duplicates.len(), 1);
        assert_eq!(deserialized.skipped_duplicates[0], "https://dup.example.com");
        assert_eq!(deserialized.skipped_invalid.len(), 1);
        assert_eq!(deserialized.skipped_invalid[0], "not-a-url");
    }

    #[test]
    fn test_import_result_stats_consistency() {
        // success_count + skipped_duplicates + skipped_invalid should equal total
        let result = ImportResult {
            imported: vec![],
            skipped_duplicates: vec!["a".to_string(), "b".to_string()],
            skipped_invalid: vec!["c".to_string()],
            total: 3,
            success_count: 0,
        };
        assert_eq!(
            result.success_count + result.skipped_duplicates.len() + result.skipped_invalid.len(),
            result.total
        );
    }

    #[test]
    fn test_opml_outline_serde_roundtrip() {
        let outline = OpmlOutline {
            title: "My Feed".to_string(),
            xml_url: "https://example.com/rss".to_string(),
            description: Some("A great feed".to_string()),
        };

        let json = serde_json::to_string(&outline).expect("serialization should succeed");
        let deserialized: OpmlOutline =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.title, "My Feed");
        assert_eq!(deserialized.xml_url, "https://example.com/rss");
        assert_eq!(deserialized.description, Some("A great feed".to_string()));
    }

    #[test]
    fn test_opml_outline_no_description() {
        let outline = OpmlOutline {
            title: "Feed".to_string(),
            xml_url: "https://example.com/rss".to_string(),
            description: None,
        };

        let json = serde_json::to_string(&outline).expect("serialization should succeed");
        let deserialized: OpmlOutline =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.title, "Feed");
        assert_eq!(deserialized.xml_url, "https://example.com/rss");
        assert_eq!(deserialized.description, None);
    }

    #[test]
    fn test_import_result_json_structure() {
        let result = ImportResult {
            imported: vec![],
            skipped_duplicates: vec![],
            skipped_invalid: vec![],
            total: 0,
            success_count: 0,
        };

        let json = serde_json::to_string_pretty(&result).expect("serialization should succeed");

        // Verify JSON structure contains all expected fields
        assert!(json.contains("\"imported\""));
        assert!(json.contains("\"skipped_duplicates\""));
        assert!(json.contains("\"skipped_invalid\""));
        assert!(json.contains("\"total\""));
        assert!(json.contains("\"success_count\""));
    }

    // ── ImportSource tests ─────────────────────────────────────────

    #[test]
    fn test_import_source_opml_file_serde() {
        let source = ImportSource::OpmlFile {
            path: "/path/to/feeds.opml".to_string(),
        };
        let json = serde_json::to_string(&source).expect("serialize OpmlFile");
        assert!(json.contains("\"type\":\"opml\""));
        assert!(json.contains("\"path\":\"/path/to/feeds.opml\""));

        let deserialized: ImportSource =
            serde_json::from_str(&json).expect("deserialize OpmlFile");
        match deserialized {
            ImportSource::OpmlFile { path } => assert_eq!(path, "/path/to/feeds.opml"),
            _ => panic!("expected OpmlFile variant"),
        }
    }

    #[test]
    fn test_import_source_txt_file_serde() {
        let source = ImportSource::TxtFile {
            path: "/path/to/urls.txt".to_string(),
        };
        let json = serde_json::to_string(&source).expect("serialize TxtFile");
        assert!(json.contains("\"type\":\"txt\""));
        assert!(json.contains("\"path\":\"/path/to/urls.txt\""));

        let deserialized: ImportSource =
            serde_json::from_str(&json).expect("deserialize TxtFile");
        match deserialized {
            ImportSource::TxtFile { path } => assert_eq!(path, "/path/to/urls.txt"),
            _ => panic!("expected TxtFile variant"),
        }
    }

    #[test]
    fn test_import_source_url_list_serde() {
        let source = ImportSource::UrlList {
            urls: vec![
                "https://youtube.com/@a".to_string(),
                "https://youtube.com/@b".to_string(),
            ],
        };
        let json = serde_json::to_string(&source).expect("serialize UrlList");
        assert!(json.contains("\"type\":\"url_list\""));
        assert!(json.contains("\"urls\""));

        let deserialized: ImportSource =
            serde_json::from_str(&json).expect("deserialize UrlList");
        match deserialized {
            ImportSource::UrlList { urls } => {
                assert_eq!(urls.len(), 2);
                assert_eq!(urls[0], "https://youtube.com/@a");
            }
            _ => panic!("expected UrlList variant"),
        }
    }

    // ── ImportPreview / ImportPreviewItem tests ────────────────────

    #[test]
    fn test_import_preview_item_serde() {
        let item = ImportPreviewItem {
            url: "https://youtube.com/@test".to_string(),
            title: Some("Test Channel".to_string()),
            is_duplicate: false,
            error: None,
        };
        let json = serde_json::to_string(&item).expect("serialize ImportPreviewItem");
        assert!(json.contains("\"url\":\"https://youtube.com/@test\""));
        assert!(json.contains("\"title\":\"Test Channel\""));
        assert!(json.contains("\"is_duplicate\":false"));
        assert!(json.contains("\"error\":null"));

        let deserialized: ImportPreviewItem =
            serde_json::from_str(&json).expect("deserialize ImportPreviewItem");
        assert_eq!(deserialized.url, "https://youtube.com/@test");
        assert_eq!(deserialized.title, Some("Test Channel".to_string()));
        assert!(!deserialized.is_duplicate);
        assert_eq!(deserialized.error, None);
    }

    #[test]
    fn test_import_preview_item_with_error() {
        let item = ImportPreviewItem {
            url: "not-a-valid-url".to_string(),
            title: None,
            is_duplicate: false,
            error: Some("Invalid URL format".to_string()),
        };
        let json = serde_json::to_string(&item).expect("serialize with error");
        assert!(json.contains("\"error\":\"Invalid URL format\""));
        assert!(json.contains("\"title\":null"));
    }

    #[test]
    fn test_import_preview_serde() {
        let preview = ImportPreview {
            items: vec![
                ImportPreviewItem {
                    url: "https://youtube.com/@a".to_string(),
                    title: Some("A".to_string()),
                    is_duplicate: false,
                    error: None,
                },
                ImportPreviewItem {
                    url: "https://youtube.com/@b".to_string(),
                    title: Some("B".to_string()),
                    is_duplicate: true,
                    error: None,
                },
            ],
            total: 2,
            duplicates: 1,
        };
        let json = serde_json::to_string(&preview).expect("serialize ImportPreview");
        assert!(json.contains("\"total\":2"));
        assert!(json.contains("\"duplicates\":1"));
        assert!(json.contains("\"is_duplicate\":true"));

        let deserialized: ImportPreview =
            serde_json::from_str(&json).expect("deserialize ImportPreview");
        assert_eq!(deserialized.total, 2);
        assert_eq!(deserialized.duplicates, 1);
        assert_eq!(deserialized.items.len(), 2);
    }

    // ── ImportProgressEvent tests ──────────────────────────────────

    #[test]
    fn test_import_progress_event_serde() {
        let event = ImportProgressEvent {
            task_id: "task-123".to_string(),
            completed: 3,
            total: 10,
            current_url: "https://youtube.com/@channel".to_string(),
            current_title: "My Channel".to_string(),
        };
        let json = serde_json::to_string(&event).expect("serialize ImportProgressEvent");
        assert!(json.contains("\"task_id\":\"task-123\""));
        assert!(json.contains("\"completed\":3"));
        assert!(json.contains("\"total\":10"));
        assert!(json.contains("\"current_url\":\"https://youtube.com/@channel\""));
        assert!(json.contains("\"current_title\":\"My Channel\""));
    }

    // ── ImportCompleteEvent tests ──────────────────────────────────

    #[test]
    fn test_import_complete_event_serde() {
        let event = ImportCompleteEvent {
            task_id: "task-123".to_string(),
            total: 5,
            success: 3,
            failed: 1,
            skipped: 1,
            errors: vec![ImportErrorItem {
                url: "https://bad.example.com".to_string(),
                reason: "DNS resolution failed".to_string(),
            }],
        };
        let json = serde_json::to_string(&event).expect("serialize ImportCompleteEvent");
        assert!(json.contains("\"success\":3"));
        assert!(json.contains("\"failed\":1"));
        assert!(json.contains("\"skipped\":1"));
        assert!(json.contains("\"DNS resolution failed\""));

        let deserialized: ImportCompleteEvent =
            serde_json::from_str(&json).expect("deserialize ImportCompleteEvent");
        assert_eq!(deserialized.total, 5);
        assert_eq!(deserialized.success, 3);
        assert_eq!(deserialized.failed, 1);
        assert_eq!(deserialized.skipped, 1);
        assert_eq!(deserialized.errors.len(), 1);
        assert_eq!(deserialized.errors[0].url, "https://bad.example.com");
    }

    // ── ImportErrorItem tests ──────────────────────────────────────

    #[test]
    fn test_import_error_item_serde() {
        let item = ImportErrorItem {
            url: "https://example.com/feed".to_string(),
            reason: "Channel not found".to_string(),
        };
        let json = serde_json::to_string(&item).expect("serialize ImportErrorItem");
        assert!(json.contains("\"url\":\"https://example.com/feed\""));
        assert!(json.contains("\"reason\":\"Channel not found\""));
    }
}
