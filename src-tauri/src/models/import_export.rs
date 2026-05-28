use serde::{Deserialize, Serialize};

use crate::models::Subscription;

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
}
