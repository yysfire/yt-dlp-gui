use serde::Serialize;

/// Parsed progress event from yt-dlp's --progress-template output.
#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub percent: f32,
    pub speed: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub eta: String,
}

/// Parse a single line from yt-dlp's --progress-template output.
///
/// The expected format is: `percent|speed|downloaded_bytes|total_bytes|eta`
/// Example: `45.2|2.3MiB/s|125800000|278400000|00:02:15`
///
/// Returns None if the line is empty or cannot be parsed.
pub fn parse_progress_line(line: &str) -> Option<ProgressEvent> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() != 5 {
        return None;
    }

    let percent: f32 = parts.first()?.parse().ok()?;
    let speed = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let downloaded_bytes: u64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let total_bytes: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let eta = parts.get(4).map(|s| s.to_string()).unwrap_or_default();

    Some(ProgressEvent {
        percent,
        speed,
        downloaded_bytes,
        total_bytes,
        eta,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_normal_progress() {
        let line = "45.2|2.3MiB/s|125800000|278400000|00:02:15";
        let result = parse_progress_line(line).expect("should parse");
        assert!((result.percent - 45.2).abs() < 0.01);
        assert_eq!(result.speed, "2.3MiB/s");
        assert_eq!(result.downloaded_bytes, 125800000);
        assert_eq!(result.total_bytes, 278400000);
        assert_eq!(result.eta, "00:02:15");
    }

    #[test]
    fn test_parse_complete() {
        let line = "100.0|0.0KiB/s|278400000|278400000|00:00:00";
        let result = parse_progress_line(line).expect("should parse");
        assert!((result.percent - 100.0).abs() < 0.01);
        assert_eq!(result.downloaded_bytes, result.total_bytes);
    }

    #[test]
    fn test_parse_empty_string() {
        assert!(parse_progress_line("").is_none());
        assert!(parse_progress_line("   ").is_none());
    }

    #[test]
    fn test_parse_malformed() {
        // Malformed input should fail gracefully
        assert!(parse_progress_line("not a progress line").is_none());
        assert!(parse_progress_line("abc|def|ghi|jkl|mno").is_none());
    }

    #[test]
    fn test_parse_missing_parts() {
        // Only percent is present — must fail with strict 5-part requirement
        let line = "50.0";
        assert!(parse_progress_line(line).is_none(), "less than 5 parts must return None");
    }

    #[test]
    fn test_parse_zero_progress() {
        let line = "0.0|0.0KiB/s|0|100000|00:10:00";
        let result = parse_progress_line(line).expect("should parse zero progress");
        assert!((result.percent - 0.0).abs() < 0.01);
        assert_eq!(result.downloaded_bytes, 0);
    }
}
