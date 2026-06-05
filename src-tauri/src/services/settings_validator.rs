//! Settings validation service.
//!
//! Pure validation functions — stateless, no Tauri runtime dependencies.
//! Each public function can be tested independently in `#[cfg(test)]`.

use std::fs;
use std::io::Write;
use std::path::Path;

/// Result of download path validation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PathValidateResult {
    pub valid: bool,
    pub writable: bool,
    pub exists: bool,
    pub error: Option<String>,
}

/// Result of proxy URL validation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProxyValidateResult {
    pub valid: bool,
    pub scheme: Option<String>,
    pub error: Option<String>,
}

/// Validates that a download directory path is accessible and writable.
pub fn validate_download_path(path: &str) -> PathValidateResult {
    let p = Path::new(path);

    if path.trim().is_empty() {
        return PathValidateResult {
            valid: false,
            writable: false,
            exists: false,
            error: Some("下载路径不能为空".to_string()),
        };
    }

    // Try to create the directory (or verify it exists)
    match fs::create_dir_all(p) {
        Ok(()) => {}
        Err(e) => {
            return PathValidateResult {
                valid: false,
                writable: false,
                exists: p.exists(),
                error: Some(format!("无法创建下载目录: {}", e)),
            };
        }
    }

    // Verify writability by writing a temp file
    let test_file = p.join(".write_test");
    match fs::File::create(&test_file) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(b"test") {
                return PathValidateResult {
                    valid: false,
                    writable: false,
                    exists: p.exists(),
                    error: Some(format!("下载路径不可写入: {}", e)),
                };
            }
            // Clean up test file
            let _ = fs::remove_file(&test_file);
            PathValidateResult {
                valid: true,
                writable: true,
                exists: p.exists(),
                error: None,
            }
        }
        Err(e) => PathValidateResult {
            valid: false,
            writable: false,
            exists: p.exists(),
            error: Some(format!("下载路径不可写入: {}", e)),
        },
    }
}

/// Validates a proxy URL format.
///
/// Accepts empty string (no proxy), and URLs with schemes:
/// http, https, socks5, socks5h
pub fn validate_proxy_url(input: &str) -> ProxyValidateResult {
    if input.trim().is_empty() {
        return ProxyValidateResult {
            valid: true,
            scheme: None,
            error: None,
        };
    }

    // Require "://" authority delimiter — yt-dlp only accepts proper authority-form URLs.
    // Without this, the url crate parses "http:127.0.0.1:6478" (missing "//") as a valid URL,
    // but yt-dlp would not treat it as a proxy.
    if !input.contains("://") {
        return ProxyValidateResult {
            valid: false,
            scheme: None,
            error: Some("代理地址格式无效：缺少 ://（例如 http://127.0.0.1:7890）".to_string()),
        };
    }

    let parsed = match url::Url::parse(input) {
        Ok(url) => url,
        Err(e) => {
            return ProxyValidateResult {
                valid: false,
                scheme: None,
                error: Some(format!("代理地址格式无效: {}", e)),
            };
        }
    };

    let scheme = parsed.scheme();
    match scheme {
        "http" | "https" | "socks5" | "socks5h" => {}
        other => {
            return ProxyValidateResult {
                valid: false,
                scheme: Some(other.to_string()),
                error: Some(format!("不支持的代理协议: {}", other)),
            };
        }
    }

    if parsed.host_str().unwrap_or("").is_empty() {
        return ProxyValidateResult {
            valid: false,
            scheme: Some(scheme.to_string()),
            error: Some("代理地址缺少主机名".to_string()),
        };
    }

    ProxyValidateResult {
        valid: true,
        scheme: Some(scheme.to_string()),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    // ── validate_download_path tests ──

    #[test]
    fn test_validate_path_empty() {
        let result = validate_download_path("");
        assert!(!result.valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_validate_path_exists_and_writable() {
        let dir = env::temp_dir().join("yt-dlp-gui-test-writable");
        fs::create_dir_all(&dir).unwrap();
        let result = validate_download_path(dir.to_str().unwrap());
        assert!(result.valid);
        assert!(result.writable);
        // Clean up
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_path_created() {
        let dir = env::temp_dir().join("yt-dlp-gui-test-new");
        // Ensure it doesn't exist
        let _ = fs::remove_dir_all(&dir);
        let result = validate_download_path(dir.to_str().unwrap());
        assert!(result.valid);
        assert!(result.writable);
        // Clean up
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_path_unwritable() {
        // On Linux, root-only directories should be unwritable
        let path = "/root/test-unwritable";
        let result = validate_download_path(path);
        // Should not be valid (non-root user can't write to /root)
        assert!(!result.valid, "Expected path {} to be unwritable", path);
    }

    #[test]
    fn test_validate_path_empty_string() {
        let result = validate_download_path("");
        assert!(!result.valid);
        assert!(!result.writable);
        assert!(result.error.is_some());
    }

    // ── validate_proxy_url tests ──

    #[test]
    fn test_validate_proxy_empty() {
        let result = validate_proxy_url("");
        assert!(result.valid);
        assert!(result.scheme.is_none());
    }

    #[test]
    fn test_validate_proxy_http() {
        let result = validate_proxy_url("http://127.0.0.1:7890");
        assert!(result.valid);
        assert_eq!(result.scheme.as_deref(), Some("http"));
    }

    #[test]
    fn test_validate_proxy_https() {
        let result = validate_proxy_url("https://proxy.example.com:8443");
        assert!(result.valid);
        assert_eq!(result.scheme.as_deref(), Some("https"));
    }

    #[test]
    fn test_validate_proxy_socks5() {
        let result = validate_proxy_url("socks5://127.0.0.1:1080");
        assert!(result.valid);
        assert_eq!(result.scheme.as_deref(), Some("socks5"));
    }

    #[test]
    fn test_validate_proxy_socks5h() {
        let result = validate_proxy_url("socks5h://127.0.0.1:1080");
        assert!(result.valid);
        assert_eq!(result.scheme.as_deref(), Some("socks5h"));
    }

    #[test]
    fn test_validate_proxy_invalid() {
        let result = validate_proxy_url("not-a-url");
        assert!(!result.valid);
    }

    #[test]
    fn test_validate_proxy_no_host() {
        let result = validate_proxy_url("http://");
        eprintln!("DEBUG valid={} error={:?} scheme={:?}", result.valid, result.error, result.scheme);
        assert!(!result.valid);
        let err = result.error.expect("expected validation error");
        assert!(err.contains("主机名") || err.contains("host"), "unexpected error: {}", err);
    }

    #[test]
    fn test_validate_proxy_unsupported_scheme() {
        let result = validate_proxy_url("ftp://proxy.example.com:21");
        assert!(!result.valid);
        assert!(result.error.unwrap().contains("不支持的代理协议"));
    }

    #[test]
    fn test_validate_path_unicode() {
        let dir = env::temp_dir().join("测试-路径");
        let _ = fs::create_dir_all(&dir);
        let result = validate_download_path(dir.to_str().unwrap());
        assert!(result.valid, "Unicode path should be valid");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_proxy_missing_double_slash() {
        // "http:127.0.0.1:6478" (missing //) should be rejected
        let result = validate_proxy_url("http:127.0.0.1:6478");
        assert!(!result.valid, "Missing '//' should be invalid");
        assert!(result.error.unwrap().contains("://"));
    }
}
