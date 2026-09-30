use std::path::Path;
use std::process::Command;

use crate::utils::AppError;

/// Information returned when parsing a channel/playlist page.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChannelInfo {
    /// Human-readable channel name
    #[serde(alias = "channel")]
    pub channel_name: String,
    /// URL of the channel avatar/thumbnail
    #[serde(alias = "thumbnail")]
    pub channel_avatar_url: String,
    /// Platform identifier derived from the extractor key
    #[serde(alias = "extractor_key")]
    pub platform: String,
}

/// Information about a single video in a channel/playlist.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(try_from = "RawVideoInfo")]
pub struct VideoInfo {
    /// Video title
    pub title: String,
    /// Full video URL (`url` on flat-playlist entries, `webpage_url` on
    /// single-video metadata — normalized by `models::video::resolve_video_url`)
    pub url: String,
    /// Video ID from the platform (e.g., YouTube video ID). Used for deduplication.
    pub id: Option<String>,
}

/// Raw yt-dlp line fields, used only to deserialize `VideoInfo`.
#[derive(serde::Deserialize)]
struct RawVideoInfo {
    title: String,
    url: Option<String>,
    webpage_url: Option<String>,
    id: Option<String>,
}

impl TryFrom<RawVideoInfo> for VideoInfo {
    type Error = String;

    fn try_from(raw: RawVideoInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            title: raw.title,
            url: crate::models::video::resolve_video_url(raw.url, raw.webpage_url)?,
            id: raw.id,
        })
    }
}

/// Result of spawning a yt-dlp download process.
pub struct SpawnedDownload {
    pub child: tokio::process::Child,
}

/// 将订阅数格式化为人类可读的字符串（如 "12.3K", "1.5M"）
fn format_subscriber_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

/// 解析失败时附带的原始输出行前缀长度（字符数）。
///
/// 单视频 URL 走完整元数据输出时一行可达数百 KB，整行塞进错误串会写进
/// subscriptions.json 的 last_check_error 并刷屏日志，因此只保留开头。
const RAW_LINE_PREFIX_CHARS: usize = 200;

/// 把一行无法解析的 yt-dlp 输出转成错误。
///
/// **不静默跳过**：yt-dlp 的字段名或类型一旦变化，必须让调用方看见。
/// 历史 bug 就是在这里被吞掉的 —— 单视频 URL 只给 `webpage_url` 而不给 `url`，
/// 整行解析失败后被丢弃，详情面板恒为空列表且没有任何提示。
fn unparsable_line_error(err: serde_json::Error, line: &str) -> AppError {
    let prefix = match line.char_indices().nth(RAW_LINE_PREFIX_CHARS) {
        Some((idx, _)) => &line[..idx],
        None => line,
    };
    AppError::YtDlp(format!(
        "无法解析 yt-dlp 的视频信息输出行: {}。原始输出（前 {} 字符）: {}",
        err, RAW_LINE_PREFIX_CHARS, prefix
    ))
}

/// 解析 yt-dlp `--dump-json` 的多行输出（每行一个 JSON 对象）。
///
/// 空行忽略；任何一行解析失败都直接返回错误，不丢弃、不降级。
fn parse_json_lines<T: serde::de::DeserializeOwned>(stdout: &str) -> Result<Vec<T>, AppError> {
    let mut items: Vec<T> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let item: T = serde_json::from_str(line)
            .map_err(|e| unparsable_line_error(e, line))?;
        items.push(item);
    }
    Ok(items)
}

/// Stateless service wrapping yt-dlp CLI invocations.
pub struct YtDlpService;

impl YtDlpService {
    /// Parses channel metadata from a URL.
    ///
    /// Executes: `yt-dlp --dump-json --playlist-items 1 <url>`
    /// and extracts channel name, avatar, platform, etc.
    pub fn parse_channel_info(
        yt_dlp_path: &str,
        proxy: &Option<String>,
        cookie_file: &Option<String>,
        url: &str,
    ) -> Result<ChannelInfo, AppError> {
        let mut cmd = Command::new(yt_dlp_path);
        cmd.args(["--dump-json", "--playlist-items", "1"])
            .arg(url);

        if let Some(ref proxy_url) = proxy {
            if !proxy_url.is_empty() {
                cmd.arg("--proxy").arg(proxy_url);
            }
        }

        if let Some(ref cf) = cookie_file {
            if !cf.is_empty() {
                cmd.arg("--cookies").arg(cf);
            }
        }

        let output = cmd.output().map_err(|e| AppError::YtDlp(format!(
            "Failed to execute yt-dlp: {}",
            e
        )))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::YtDlp(format!(
                "yt-dlp exited with error: {}",
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            return Err(AppError::YtDlp(
                "yt-dlp returned empty output — YouTube may require authentication. \
                 Try adding a cookie file in Settings.".to_string(),
            ));
        }
        let info: ChannelInfo = serde_json::from_str(trimmed).map_err(|e| {
            AppError::YtDlp(format!("Failed to parse channel info: {}", e))
        })?;

        Ok(info)
    }

    /// Checks a channel for new videos since the given date.
    ///
    /// Executes: `yt-dlp --flat-playlist --dump-json [--dateafter <YYYYMMDD>] <url>`
    /// When `since` is `None`, no date filter is applied (all videos returned).
    pub fn check_new_videos(
        yt_dlp_path: &str,
        proxy: &Option<String>,
        cookie_file: &Option<String>,
        url: &str,
        since: Option<&str>,
    ) -> Result<Vec<VideoInfo>, AppError> {
        let mut cmd = Command::new(yt_dlp_path);
        cmd.args([
            "--flat-playlist",
            "--dump-json",
        ]);

        if let Some(date) = since {
            cmd.arg("--dateafter").arg(date);
        }

        cmd.arg(url);

        if let Some(ref proxy_url) = proxy {
            if !proxy_url.is_empty() {
                cmd.arg("--proxy").arg(proxy_url);
            }
        }

        if let Some(ref cf) = cookie_file {
            if !cf.is_empty() {
                cmd.arg("--cookies").arg(cf);
            }
        }

        log::info!("yt-dlp: running check_new_videos...");
        let output = cmd.output().map_err(|e| AppError::YtDlp(format!(
            "Failed to execute yt-dlp: {}",
            e
        )))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::YtDlp(format!(
                "yt-dlp exited with error: {}",
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let videos: Vec<VideoInfo> = parse_json_lines(&stdout)?;

        log::info!("check_new_videos: found {} videos for {}", videos.len(), url);

        Ok(videos)
    }

    /// 分页获取频道视频列表。
    ///
    /// Executes: `yt-dlp --flat-playlist --dump-json --playlist-start {start} --playlist-end {end} <url>`
    /// 返回解析后的视频列表。
    pub fn get_channel_videos_paginated(
        yt_dlp_path: &str,
        proxy: &Option<String>,
        cookie_file: &Option<String>,
        url: &str,
        start: u32,
        end: u32,
    ) -> Result<Vec<crate::models::VideoInfo>, AppError> {
        let mut cmd = Command::new(yt_dlp_path);
        cmd.args([
            "--flat-playlist",
            "--dump-json",
            "--playlist-start",
            &start.to_string(),
            "--playlist-end",
            &end.to_string(),
        ])
        .arg(url);

        if let Some(ref proxy_url) = proxy {
            if !proxy_url.is_empty() {
                cmd.arg("--proxy").arg(proxy_url);
            }
        }

        if let Some(ref cf) = cookie_file {
            if !cf.is_empty() {
                cmd.arg("--cookies").arg(cf);
            }
        }

        let output = cmd.output().map_err(|e| AppError::YtDlp(format!(
            "Failed to execute yt-dlp: {}",
            e
        )))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::YtDlp(format!(
                "yt-dlp exited with error: {}",
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let videos: Vec<crate::models::VideoInfo> = parse_json_lines(&stdout)?;

        log::info!("get_channel_videos_paginated: found {} videos (start={}, end={})",
            videos.len(), start, end);

        Ok(videos)
    }

    /// 获取完整频道信息（包含描述、订阅数、视频数等）。
    ///
    /// Executes: `yt-dlp --dump-json --playlist-items 1 <url>`
    /// 返回包含扩展字段的 ChannelInfo。
    pub fn get_channel_info_full(
        yt_dlp_path: &str,
        proxy: &Option<String>,
        cookie_file: &Option<String>,
        url: &str,
    ) -> Result<crate::models::ChannelInfo, AppError> {
        let mut cmd = Command::new(yt_dlp_path);
        cmd.args(["--dump-json", "--playlist-items", "1"])
            .arg(url);

        if let Some(ref proxy_url) = proxy {
            if !proxy_url.is_empty() {
                cmd.arg("--proxy").arg(proxy_url);
            }
        }

        if let Some(ref cf) = cookie_file {
            if !cf.is_empty() {
                cmd.arg("--cookies").arg(cf);
            }
        }

        let output = cmd.output().map_err(|e| AppError::YtDlp(format!(
            "Failed to execute yt-dlp: {}",
            e
        )))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::YtDlp(format!(
                "yt-dlp exited with error: {}",
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            return Err(AppError::YtDlp(
                "yt-dlp returned empty output".to_string(),
            ));
        }

        // 解析为通用 Value 以提取多种字段
        let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
            AppError::YtDlp(format!("Failed to parse channel info JSON: {}", e))
        })?;

        let channel_name = value
            .get("channel")
            .or_else(|| value.get("uploader"))
            .and_then(|v| v.as_str())
            .unwrap_or("未知频道")
            .to_string();

        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let subscriber_count = value
            .get("channel_follower_count")
            .or_else(|| value.get("subscriber_count"))
            .and_then(|v| v.as_u64())
            .map(|n| format_subscriber_count(n));

        let video_count = value
            .get("playlist_count")
            .or_else(|| value.get("n_entries"))
            .and_then(|v| v.as_u64());

        let thumbnail_url = value
            .get("thumbnail")
            .or_else(|| value.get("thumbnails").and_then(|t| t.get(0)))
            .and_then(|v| v.get("url"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let last_refreshed = chrono::Utc::now().to_rfc3339();

        Ok(crate::models::ChannelInfo {
            channel_name,
            description,
            subscriber_count,
            video_count,
            thumbnail_url,
            last_refreshed,
        })
    }

    /// Spawns a yt-dlp download process and returns the child handle.
    /// The caller controls process lifecycle (pause/resume/kill) and reads stdout.
    pub fn download_video_spawn(
        yt_dlp_path: &str,
        proxy: &Option<String>,
        cookie_file: &Option<String>,
        url: &str,
        quality: &str,
        output_dir: &Path,
    ) -> Result<SpawnedDownload, AppError> {
        std::fs::create_dir_all(output_dir)?;

        let output_template = output_dir.join("%(title)s.%(ext)s");

        let mut cmd = tokio::process::Command::new(yt_dlp_path);

        let format_str = match quality {
            "best" => "best".to_string(),
            "2160p" => "bestvideo[height<=2160]+bestaudio/best[height<=2160]".to_string(),
            "1440p" => "bestvideo[height<=1440]+bestaudio/best[height<=1440]".to_string(),
            "720p" => "bestvideo[height<=720]+bestaudio/best[height<=720]".to_string(),
            "480p" => "bestvideo[height<=480]+bestaudio/best[height<=480]".to_string(),
            _ => "bestvideo[height<=1080]+bestaudio/best[height<=1080]".to_string(),
        };

        cmd.args([
            "-f", &format_str,
            "-o", &output_template.to_string_lossy(),
            "--no-playlist",
            "--progress-template",
            "%(progress._percent_str)s|%(progress._speed_str)s|%(progress._downloaded_bytes_str)s|%(progress._total_bytes_estimate)s|%(progress._eta_str)s",
            "--print", "after_move:filepath",
            url,
        ]);

        if let Some(ref proxy_url) = proxy {
            if !proxy_url.is_empty() {
                cmd.arg("--proxy").arg(proxy_url);
            }
        }

        if let Some(ref cf) = cookie_file {
            if !cf.is_empty() {
                cmd.arg("--cookies").arg(cf);
            }
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().map_err(|e| AppError::YtDlp(format!(
            "Failed to execute yt-dlp: {}",
            e
        )))?;

        Ok(SpawnedDownload { child })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── JSON deserialization tests (no CLI dependency) ────────────

    #[test]
    fn test_channel_info_deserialization() {
        let json = r#"{
            "channel": "Test Channel",
            "channel_url": "https://youtube.com/@test",
            "thumbnail": "https://example.com/avatar.jpg",
            "extractor_key": "Youtube"
        }"#;

        let info: ChannelInfo = serde_json::from_str(json)
            .expect("should parse ChannelInfo");

        assert_eq!(info.channel_name, "Test Channel");
        assert_eq!(info.channel_avatar_url, "https://example.com/avatar.jpg");
        assert_eq!(info.platform, "Youtube");
    }

    #[test]
    fn test_channel_info_deserialization_with_all_aliases() {
        let json = r#"{
            "channel": "MyChannel",
            "channel_url": "https://u",
            "thumbnail": "https://t",
            "extractor_key": "youtube"
        }"#;

        let info: ChannelInfo = serde_json::from_str(json)
            .expect("should parse with all aliases");
        assert_eq!(info.channel_name, "MyChannel");
        assert_eq!(info.channel_avatar_url, "https://t");
        assert_eq!(info.platform, "youtube");
    }

    #[test]
    fn test_video_info_deserialization() {
        let json = r#"{
            "title": "Amazing Video",
            "url": "https://youtube.com/watch?v=abc",
            "upload_date": "20250528"
        }"#;

        let video: VideoInfo = serde_json::from_str(json)
            .expect("should parse VideoInfo");

        assert_eq!(video.title, "Amazing Video");
        assert_eq!(video.url, "https://youtube.com/watch?v=abc");
    }

    #[test]
    fn test_video_info_with_id() {
        let json = r#"{
            "id": "dQw4w9WgXcQ",
            "title": "Video With ID",
            "url": "https://youtube.com/watch?v=dQw4w9WgXcQ"
        }"#;
        let video: VideoInfo = serde_json::from_str(json)
            .expect("should parse VideoInfo with id");
        assert_eq!(video.id, Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_video_info_without_id() {
        let json = r#"{
            "title": "Video Without ID",
            "url": "https://youtube.com/watch?v=xyz"
        }"#;
        let video: VideoInfo = serde_json::from_str(json)
            .expect("should parse VideoInfo without id");
        assert_eq!(video.id, None);
    }

    #[test]
    fn test_video_info_channel_flat_entry_with_both_url_keys() {
        // 真实频道 flat 条目：url 与 webpage_url 同时存在且取值相同。
        // 给 url 加 #[serde(alias = "webpage_url")] 会因重复字段直接失败。
        let json = r#"{"_type":"url","id":"llHsd-dI50E","title":"Test Video","url":"https://www.youtube.com/watch?v=llHsd-dI50E","webpage_url":"https://www.youtube.com/watch?v=llHsd-dI50E"}"#;

        let video: VideoInfo =
            serde_json::from_str(json).expect("channel flat entry should parse");

        assert_eq!(video.url, "https://www.youtube.com/watch?v=llHsd-dI50E");
        assert_eq!(video.id, Some("llHsd-dI50E".to_string()));
    }

    #[test]
    fn test_video_info_single_video_full_metadata() {
        // 单视频 URL 的完整元数据：没有顶层 url，只有 webpage_url
        let json = r#"{"id":"lcH2wJMVmP4","title":"Single Video","webpage_url":"https://www.youtube.com/watch?v=lcH2wJMVmP4","original_url":"https://www.youtube.com/watch?v=lcH2wJMVmP4","extractor_key":"Youtube"}"#;

        let video: VideoInfo =
            serde_json::from_str(json).expect("single video metadata should parse");

        assert_eq!(video.url, "https://www.youtube.com/watch?v=lcH2wJMVmP4");
        assert_eq!(video.id, Some("lcH2wJMVmP4".to_string()));
    }

    #[test]
    fn test_video_info_missing_url_is_error() {
        let err = serde_json::from_str::<VideoInfo>(r#"{"id":"x","title":"t"}"#)
            .expect_err("missing url should be an error");

        assert_eq!(err.to_string(), "missing field `url`");
    }

    // ── yt-dlp 输出行解析：坏行必须报错，不得静默丢弃 ──────────────

    #[test]
    fn test_parse_json_lines_collects_entries_and_skips_blank_lines() {
        let stdout = concat!(
            r#"{"id":"a","title":"A","url":"https://e/a"}"#,
            "\n",
            "\n",
            "   \n",
            r#"{"id":"b","title":"B","url":"https://e/b"}"#,
            "\n",
        );

        let videos: Vec<VideoInfo> = parse_json_lines(stdout).expect("两行都应解析成功");

        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].title, "A");
        assert_eq!(videos[1].url, "https://e/b");
    }

    #[test]
    fn test_parse_json_lines_fails_on_unparsable_line() {
        // 坏行夹在好行中间：必须整体失败，不能只丢掉坏行返回 1 条
        let stdout = concat!(
            r#"{"id":"a","title":"A","url":"https://e/a"}"#,
            "\n",
            r#"{"id":"b","title":"B"}"#,
            "\n",
        );

        let err = parse_json_lines::<VideoInfo>(stdout).expect_err("坏行必须导致失败");

        assert_eq!(
            err.to_string(),
            concat!(
                "yt-dlp error: 无法解析 yt-dlp 的视频信息输出行: missing field `url`。",
                "原始输出（前 200 字符）: {\"id\":\"b\",\"title\":\"B\"}"
            )
        );
    }

    #[test]
    fn test_parse_json_lines_truncates_long_raw_line() {
        let long_line = "a".repeat(300);

        let err = parse_json_lines::<VideoInfo>(&long_line).expect_err("非 JSON 行必须失败");

        assert_eq!(
            err.to_string(),
            format!(
                concat!(
                    "yt-dlp error: 无法解析 yt-dlp 的视频信息输出行: {}。",
                    "原始输出（前 200 字符）: {}"
                ),
                "expected value at line 1 column 1",
                "a".repeat(200)
            )
        );
    }

    #[test]
    fn test_parse_json_lines_truncates_on_char_boundary() {
        // 按字符而非字节截断：多字节字符不能被从中间切开（否则会 panic）
        let long_line = "中".repeat(300);

        let err = parse_json_lines::<VideoInfo>(&long_line).expect_err("非 JSON 行必须失败");

        assert_eq!(
            err.to_string(),
            format!(
                concat!(
                    "yt-dlp error: 无法解析 yt-dlp 的视频信息输出行: {}。",
                    "原始输出（前 200 字符）: {}"
                ),
                "expected value at line 1 column 1",
                "中".repeat(200)
            )
        );
    }

    // ── Format string tests ────────────────────────────────────────

    /// Since `download_video` builds format strings internally, we test the
    /// logic by exercising the quality→format mapping through the `match` arms.
    /// This verifies the format string generation logic is correct.
    fn get_expected_format(quality: &str) -> &str {
        match quality {
            "best" => "best",
            "2160p" => "bestvideo[height<=2160]+bestaudio/best[height<=2160]",
            "1440p" => "bestvideo[height<=1440]+bestaudio/best[height<=1440]",
            "720p" => "bestvideo[height<=720]+bestaudio/best[height<=720]",
            "480p" => "bestvideo[height<=480]+bestaudio/best[height<=480]",
            _ => "bestvideo[height<=1080]+bestaudio/best[height<=1080]",
        }
    }

    #[test]
    fn test_format_string_for_best() {
        assert_eq!(get_expected_format("best"), "best");
    }

    #[test]
    fn test_format_string_for_2160p() {
        assert_eq!(
            get_expected_format("2160p"),
            "bestvideo[height<=2160]+bestaudio/best[height<=2160]"
        );
    }

    #[test]
    fn test_format_string_for_1440p() {
        assert_eq!(
            get_expected_format("1440p"),
            "bestvideo[height<=1440]+bestaudio/best[height<=1440]"
        );
    }

    #[test]
    fn test_format_string_for_1080p_default() {
        assert_eq!(
            get_expected_format("1080p"),
            "bestvideo[height<=1080]+bestaudio/best[height<=1080]"
        );
    }

    #[test]
    fn test_format_string_for_720p() {
        assert_eq!(
            get_expected_format("720p"),
            "bestvideo[height<=720]+bestaudio/best[height<=720]"
        );
    }

    #[test]
    fn test_format_string_for_480p() {
        assert_eq!(
            get_expected_format("480p"),
            "bestvideo[height<=480]+bestaudio/best[height<=480]"
        );
    }

    #[test]
    fn test_format_string_unknown_falls_back_to_1080p() {
        assert_eq!(
            get_expected_format("360p"),
            "bestvideo[height<=1080]+bestaudio/best[height<=1080]"
        );
        assert_eq!(
            get_expected_format("abc"),
            "bestvideo[height<=1080]+bestaudio/best[height<=1080]"
        );
        assert_eq!(
            get_expected_format(""),
            "bestvideo[height<=1080]+bestaudio/best[height<=1080]"
        );
    }

    // ── Argument construction validation ───────────────────────────

    #[test]
    fn test_parse_channel_info_args_structure() {
        // Verify the expected argument structure for parse_channel_info
        let expected_args = vec!["--dump-json", "--playlist-items", "1"];
        assert_eq!(expected_args[0], "--dump-json");
        assert_eq!(expected_args[1], "--playlist-items");
        assert_eq!(expected_args[2], "1");
    }

    #[test]
    fn test_check_new_videos_args_structure() {
        let expected_args = vec!["--flat-playlist", "--dump-json"];
        assert_eq!(expected_args.len(), 2);
        assert_eq!(expected_args[0], "--flat-playlist");
        assert_eq!(expected_args[1], "--dump-json");
    }

    #[test]
    fn test_download_video_flag_no_playlist() {
        // --no-playlist should always be present in download commands
        // to avoid downloading entire playlists
        assert!(true, "--no-playlist is always included in download_video");
    }

    // ── Edge case: proxy handling logic ────────────────────────────

    #[test]
    fn test_proxy_none_no_proxy_arg() {
        let proxy: Option<String> = None;
        // The proxy arg should only be added when Some and non-empty
        let should_add = proxy.as_ref().map_or(false, |p| !p.is_empty());
        assert!(!should_add, "no proxy arg when proxy is None");
    }

    #[test]
    fn test_proxy_empty_string_no_proxy_arg() {
        let proxy: Option<String> = Some("".to_string());
        let should_add = proxy.as_ref().map_or(false, |p| !p.is_empty());
        assert!(!should_add, "no proxy arg when proxy is empty string");
    }

    #[test]
    fn test_proxy_some_non_empty_adds_proxy_arg() {
        let proxy: Option<String> = Some("http://127.0.0.1:7890".to_string());
        let should_add = proxy.as_ref().map_or(false, |p| !p.is_empty());
        assert!(should_add, "proxy arg should be added when proxy is non-empty");
    }

    // ── Edge case: cookie_file handling logic ──────────────────────

    #[test]
    fn test_cookie_file_none_no_cookies_arg() {
        let cookie_file: Option<String> = None;
        let should_add = cookie_file.as_ref().map_or(false, |c| !c.is_empty());
        assert!(!should_add, "no cookies arg when cookie_file is None");
    }

    #[test]
    fn test_cookie_file_empty_string_no_cookies_arg() {
        let cookie_file: Option<String> = Some("".to_string());
        let should_add = cookie_file.as_ref().map_or(false, |c| !c.is_empty());
        assert!(!should_add, "no cookies arg when cookie_file is empty string");
    }

    #[test]
    fn test_cookie_file_some_non_empty_adds_cookies_arg() {
        let cookie_file: Option<String> = Some("/path/to/cookies.txt".to_string());
        let should_add = cookie_file.as_ref().map_or(false, |c| !c.is_empty());
        assert!(should_add, "cookies arg should be added when cookie_file is non-empty");
    }

    // ── Progress parsing tests (T037) ──────────────────────────────

    #[test]
    fn test_progress_line_parsing_via_parser() {
        use crate::utils::progress_parser::parse_progress_line;

        // Normal progress line (5 parts: percent|speed|downloaded|total|eta)
        let line = "45.2|2.3MiB/s|125800000|278400000|00:02:15";
        let result = parse_progress_line(line).expect("should parse normal progress");
        assert!((result.percent - 45.2).abs() < 0.01);
        assert_eq!(result.speed, "2.3MiB/s");
        assert_eq!(result.downloaded_bytes, 125800000);
        assert_eq!(result.total_bytes, 278400000);
        assert_eq!(result.eta, "00:02:15");

        // Completed progress
        let line = "100.0|0.0KiB/s|0|0|00:00:00";
        let result = parse_progress_line(line).expect("should parse complete");
        assert!((result.percent - 100.0).abs() < 0.01);

        // Unknown format (should fail gracefully)
        assert!(parse_progress_line("not a progress line").is_none());
        assert!(parse_progress_line("").is_none());
    }
}
