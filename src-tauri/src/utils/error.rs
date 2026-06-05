use thiserror::Error;

/// Unified error type for all application layers.
///
/// Commands convert this into a user-readable `String` for the frontend.
#[derive(Debug, Error)]
pub enum AppError {
    /// Filesystem or I/O errors (read/write/create directory)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization errors
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// yt-dlp CLI execution failures (captures stderr)
    #[error("yt-dlp error: {0}")]
    YtDlp(String),

    /// Requested resource was not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Duplicate resource detected (e.g., already-subscribed URL)
    #[error("Duplicate: {0}")]
    Duplicate(String),

    /// Invalid download path (doesn't exist, not writable, etc.)
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Invalid proxy URL format
    #[error("Invalid proxy: {0}")]
    InvalidProxy(String),
}
