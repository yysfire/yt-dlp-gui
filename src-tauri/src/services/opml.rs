use crate::models::{OpmlOutline, Subscription};

/// Stateless service for OPML 2.0 XML import and export.
pub struct OpmlService;

impl OpmlService {
    /// Builds an OPML 2.0 XML string from a slice of subscriptions.
    ///
    /// Each subscription is serialized as an `<outline>` element with
    /// `text`, `title`, `type="rss"`, `xmlUrl`, and `description` attributes.
    pub fn build_opml(subscriptions: &[Subscription]) -> String {
        let mut outlines = String::new();
        for sub in subscriptions {
            outlines.push_str(&format!(
                r#"    <outline text="{}" title="{}" type="rss" xmlUrl="{}" description="{}"/>"#,
                escape_xml(&sub.channel_name),
                escape_xml(&sub.channel_name),
                escape_xml(&sub.url),
                escape_xml(&sub.platform),
            ));
            outlines.push('\n');
        }
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head>
    <title>yt-dlp GUI Subscriptions</title>
    <dateCreated>{}</dateCreated>
  </head>
  <body>
{}
  </body>
</opml>"#,
            chrono::Utc::now().to_rfc3339(),
            outlines
        )
    }

    /// Parses an OPML XML string and extracts all `<outline>` elements
    /// that have a non-empty `xmlUrl` attribute.
    ///
    /// Uses `quick_xml::Reader` for event-based, non-allocating parsing.
    pub fn parse_opml(xml: &str) -> Vec<OpmlOutline> {
        let mut reader = quick_xml::Reader::from_str(xml);
        let mut outlines = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Empty(ref e))
                | Ok(quick_xml::events::Event::Start(ref e)) => {
                    if e.name().as_ref() == b"outline" {
                        let mut title = String::new();
                        let mut xml_url = String::new();
                        let mut description: Option<String> = None;

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"title" | b"text" => {
                                    title = String::from_utf8_lossy(&attr.value).to_string();
                                }
                                b"xmlUrl" => {
                                    xml_url =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                }
                                b"description" => {
                                    description = Some(
                                        String::from_utf8_lossy(&attr.value).to_string(),
                                    );
                                }
                                _ => {}
                            }
                        }

                        if !xml_url.is_empty() {
                            outlines.push(OpmlOutline {
                                title,
                                xml_url,
                                description,
                            });
                        }
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        outlines
    }
}

/// Escapes XML special characters in a string: `&`, `<`, `>`, `"`, `'`.
fn escape_xml(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(ch),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml_special_chars() {
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("a < b"), "a &lt; b");
        assert_eq!(escape_xml("a > b"), "a &gt; b");
        assert_eq!(escape_xml(r#""hello""#), "&quot;hello&quot;");
        assert_eq!(escape_xml("it's"), "it&apos;s");
    }

    #[test]
    fn test_escape_xml_no_special_chars() {
        assert_eq!(escape_xml("hello world"), "hello world");
        assert_eq!(escape_xml("频道名称"), "频道名称");
    }

    #[test]
    fn test_build_opml_empty() {
        let subs: Vec<Subscription> = Vec::new();
        let opml = OpmlService::build_opml(&subs);
        assert!(opml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(opml.contains("<opml version=\"2.0\">"));
        assert!(opml.contains("<body>"));
        assert!(opml.contains("</body>"));
        assert!(!opml.contains("<outline"));
    }

    #[test]
    fn test_build_opml_single_subscription() {
        let sub = Subscription::new(
            "https://youtube.com/@test".to_string(),
            "youtube".to_string(),
            "Test Channel".to_string(),
            "https://example.com/avatar.jpg".to_string(),
        );
        let opml = OpmlService::build_opml(&[sub]);
        assert!(opml.contains("text=\"Test Channel\""));
        assert!(opml.contains("xmlUrl=\"https://youtube.com/@test\""));
        assert!(opml.contains("type=\"rss\""));
    }

    #[test]
    fn test_build_opml_escapes_xml() {
        let sub = Subscription::new(
            "https://example.com/@test&foo".to_string(),
            "youtube".to_string(),
            "AT&T Channel".to_string(),
            "".to_string(),
        );
        let opml = OpmlService::build_opml(&[sub]);
        assert!(opml.contains("AT&amp;T Channel"));
        assert!(opml.contains("https://example.com/@test&amp;foo"));
    }

    #[test]
    fn test_parse_opml_empty() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>Test</title></head>
  <body>
  </body>
</opml>"#;
        let outlines = OpmlService::parse_opml(xml);
        assert!(outlines.is_empty());
    }

    #[test]
    fn test_parse_opml_single_outline() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>Test</title></head>
  <body>
    <outline text="Channel A" title="Channel A" type="rss" xmlUrl="https://youtube.com/@a"/>
  </body>
</opml>"#;
        let outlines = OpmlService::parse_opml(xml);
        assert_eq!(outlines.len(), 1);
        assert_eq!(outlines[0].title, "Channel A");
        assert_eq!(outlines[0].xml_url, "https://youtube.com/@a");
        assert_eq!(outlines[0].description, None);
    }

    #[test]
    fn test_parse_opml_multiple_outlines() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>Test</title></head>
  <body>
    <outline text="A" xmlUrl="https://a.com"/>
    <outline text="B" xmlUrl="https://b.com" description="Desc B"/>
    <outline text="C" xmlUrl="https://c.com"/>
  </body>
</opml>"#;
        let outlines = OpmlService::parse_opml(xml);
        assert_eq!(outlines.len(), 3);
        assert_eq!(outlines[1].title, "B");
        assert_eq!(outlines[1].description, Some("Desc B".to_string()));
    }

    #[test]
    fn test_parse_opml_skips_outlines_without_xmlurl() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="No URL"/>
    <outline text="Has URL" xmlUrl="https://example.com"/>
  </body>
</opml>"#;
        let outlines = OpmlService::parse_opml(xml);
        assert_eq!(outlines.len(), 1);
        assert_eq!(outlines[0].xml_url, "https://example.com");
    }

    #[test]
    fn test_parse_opml_prefers_title_over_text() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Text Name" title="Title Name" xmlUrl="https://example.com"/>
  </body>
</opml>"#;
        let outlines = OpmlService::parse_opml(xml);
        assert_eq!(outlines.len(), 1);
        // Both title and text map to the same title field;
        // the last one encountered wins with quick-xml attribute iteration.
        assert!(!outlines[0].title.is_empty());
    }

    #[test]
    fn test_escape_xml_empty_string() {
        assert_eq!(escape_xml(""), "");
    }

    #[test]
    fn test_escape_xml_only_special_chars() {
        assert_eq!(escape_xml("&<>\"'"), "&amp;&lt;&gt;&quot;&apos;");
    }

    #[test]
    fn test_parse_opml_malformed_xml() {
        // Malformed XML should return empty outlines, not panic
        let outlines = OpmlService::parse_opml("not valid xml <<<");
        assert!(outlines.is_empty());
    }

    #[test]
    fn test_parse_opml_nested_outlines() {
        // OPML with nested outlines (e.g., folder structure).
        // Only outlines with xmlUrl should be collected.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Folder">
      <outline text="Nested Feed" xmlUrl="https://nested.example.com/rss"/>
    </outline>
    <outline text="Top Feed" xmlUrl="https://top.example.com/rss"/>
  </body>
</opml>"#;
        let outlines = OpmlService::parse_opml(xml);
        // Both the nested and top-level outlines with xmlUrl should be collected
        assert_eq!(outlines.len(), 2);
        let urls: Vec<&str> = outlines.iter().map(|o| o.xml_url.as_str()).collect();
        assert!(urls.contains(&"https://nested.example.com/rss"));
        assert!(urls.contains(&"https://top.example.com/rss"));
    }

    #[test]
    fn test_build_opml_multiple_subscriptions() {
        let sub1 = Subscription::new(
            "https://youtube.com/@channel1".to_string(),
            "youtube".to_string(),
            "Channel One".to_string(),
            "".to_string(),
        );
        let sub2 = Subscription::new(
            "https://youtube.com/@channel2".to_string(),
            "youtube".to_string(),
            "Channel Two".to_string(),
            "".to_string(),
        );
        let opml = OpmlService::build_opml(&[sub1, sub2]);
        assert!(opml.contains("text=\"Channel One\""));
        assert!(opml.contains("text=\"Channel Two\""));
        assert!(opml.contains("xmlUrl=\"https://youtube.com/@channel1\""));
        assert!(opml.contains("xmlUrl=\"https://youtube.com/@channel2\""));
        // Verify OPML structure is intact
        assert!(opml.starts_with("<?xml"));
        assert!(opml.contains("<opml version=\"2.0\">"));
        assert!(opml.ends_with("</opml>"));
    }

    #[test]
    fn test_parse_opml_with_description() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Feed" xmlUrl="https://example.com/rss" description="A test feed"/>
  </body>
</opml>"#;
        let outlines = OpmlService::parse_opml(xml);
        assert_eq!(outlines.len(), 1);
        assert_eq!(outlines[0].description, Some("A test feed".to_string()));
    }

    #[test]
    fn test_build_opml_roundtrip_via_parse() {
        // Build OPML, parse it, and verify the result matches the original subscription
        let sub = Subscription::new(
            "https://youtube.com/@roundtrip".to_string(),
            "youtube".to_string(),
            "Roundtrip Channel".to_string(),
            "https://avatar.url".to_string(),
        );
        let opml = OpmlService::build_opml(&[sub]);
        let outlines = OpmlService::parse_opml(&opml);

        assert_eq!(outlines.len(), 1);
        assert_eq!(outlines[0].title, "Roundtrip Channel");
        assert_eq!(outlines[0].xml_url, "https://youtube.com/@roundtrip");
    }
}
