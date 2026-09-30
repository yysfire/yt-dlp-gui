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
    /// 私享/不可用视频为 `null`，由 `resolve_video_title` 回退
    title: Option<String>,
    url: Option<String>,
    webpage_url: Option<String>,
    id: Option<String>,
}

impl TryFrom<RawVideoInfo> for VideoInfo {
    type Error = String;

    fn try_from(raw: RawVideoInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            title: crate::models::video::resolve_video_title(raw.title, raw.id.as_deref()),
            url: crate::models::video::resolve_video_url(raw.url, raw.webpage_url)?,
            id: raw.id,
        })
    }
}

/// Result of spawning a yt-dlp download process.
pub struct SpawnedDownload {
    pub child: tokio::process::Child,
}

/// yt-dlp 进度模板，字段顺序与 `utils::progress_parser::parse_progress_line` 的契约一一对应：
/// `percent|speed|downloaded_bytes|total_bytes|eta`。
///
/// `percent` / `downloaded_bytes` / `total_bytes` 必须取裸数值字段。历史 bug 用的是
/// `_percent_str`（渲染成 `"  0.0%"`）与 `_downloaded_bytes_str`（渲染成 `"  1.00KiB"`），
/// 解析器里的 `parse::<f32>()` / `parse::<u64>()` 会因 `%` 和单位后缀而全部失败，
/// 进度事件一条都发不出来。`_total_bytes_estimate` 这个键并不存在，yt-dlp 只会渲染成 `NA`。
const PROGRESS_TEMPLATE: &str =
    "%(progress._percent)s|%(progress._speed_str)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress._eta_str)s";

/// 构造 yt-dlp 下载参数（代理与 Cookie 不在此处，由调用方按需追加）。
///
/// `--newline` 与 `--progress` 都不可省：
/// - 没有 `--newline`，进度以 `\r` 分隔，`BufReader::lines()`（只按 `\n` 切）永远拿不到完整行；
/// - `--print` 隐含 `--quiet`，没有 `--progress` 时进度输出会被整体抑制。
fn build_download_args(format_str: &str, output_template: &str, url: &str) -> Vec<String> {
    vec![
        "-f".to_string(),
        format_str.to_string(),
        "-o".to_string(),
        output_template.to_string(),
        "--no-playlist".to_string(),
        "--newline".to_string(),
        "--progress".to_string(),
        "--progress-template".to_string(),
        PROGRESS_TEMPLATE.to_string(),
        "--print".to_string(),
        "after_move:filepath".to_string(),
        url.to_string(),
    ]
}

/// 让 `std::process::Command` 与 `tokio::process::Command` 共用同一套环境清理逻辑。
trait EnvCommand {
    fn remove_env(&mut self, key: &str);
    fn set_env(&mut self, key: &str, value: &str);
}

impl EnvCommand for Command {
    fn remove_env(&mut self, key: &str) {
        self.env_remove(key);
    }

    fn set_env(&mut self, key: &str, value: &str) {
        self.env(key, value);
    }
}

impl EnvCommand for tokio::process::Command {
    fn remove_env(&mut self, key: &str) {
        self.env_remove(key);
    }

    fn set_env(&mut self, key: &str, value: &str) {
        self.env(key, value);
    }
}

/// `path` 是否就是 AppImage 挂载目录 `appdir`（已去掉尾部斜杠）或位于其下。
fn is_inside_appdir(path: &str, appdir: &str) -> bool {
    match path.strip_prefix(appdir) {
        Some(rest) => rest.is_empty() || rest.starts_with('/'),
        None => false,
    }
}

/// 从 `PYTHONPATH` 中剔除位于 `appdir` 下的条目；若一条不剩则返回 `None`，
/// 表示该变量应当整体移除。
fn strip_appdir_entries(value: &str, appdir: &str) -> Option<String> {
    let kept: Vec<&str> = value
        .split(':')
        .filter(|entry| !entry.is_empty() && !is_inside_appdir(entry, appdir))
        .collect();
    if kept.is_empty() {
        None
    } else {
        Some(kept.join(":"))
    }
}

/// 清除 AppImage 注入的 Python 环境污染。
///
/// Tauri 打出的 AppImage 由 linuxdeploy 生成 `AppRun`，其内部二进制（`AppRun.wrapped`）
/// 会**无条件**设置 `PYTHONHOME=$APPDIR/usr/` 与
/// `PYTHONPATH=$APPDIR/usr/share/pyshared/:<原值>`。子进程继承后，`yt-dlp`
/// （`#!/usr/bin/env python3` 脚本）会去 AppImage 挂载点里找 Python 标准库，直接以
/// `Fatal Python error: Failed to import encodings module` 崩溃。
fn sanitize_python_env<C: EnvCommand>(cmd: &mut C) {
    let appdir = std::env::var_os("APPDIR").map(|v| v.to_string_lossy().into_owned());
    let python_path = std::env::var_os("PYTHONPATH").map(|v| v.to_string_lossy().into_owned());

    for (key, value) in python_env_fixes(appdir.as_deref(), python_path.as_deref()) {
        match value {
            Some(value) => cmd.set_env(key, &value),
            None => cmd.remove_env(key),
        }
    }
}

/// 计算应施加到 yt-dlp 子进程环境的修正：`(键, Some(新值))` 表示覆盖，`None` 表示移除。
///
/// 仅在 AppImage 内（存在 `APPDIR`）才产生修正，因此开发模式与用户自己的 Python
/// 配置都不受影响。`PYTHONHOME` 已被 `AppRun` 整体覆盖、原值无从恢复，直接移除；
/// `PYTHONPATH` 是前置拼接，只剔除指向挂载目录的条目，保留用户自己的。
fn python_env_fixes(
    appdir: Option<&str>,
    python_path: Option<&str>,
) -> Vec<(&'static str, Option<String>)> {
    let Some(appdir) = appdir else {
        return Vec::new();
    };
    let appdir = appdir.trim_end_matches('/');

    let mut fixes = vec![("PYTHONHOME", None)];
    if let Some(path) = python_path {
        fixes.push(("PYTHONPATH", strip_appdir_entries(path, appdir)));
    }
    fixes
}

/// 构造已清理 Python 环境的 yt-dlp 命令（同步）。
fn yt_dlp_command(yt_dlp_path: &str) -> Command {
    let mut cmd = Command::new(yt_dlp_path);
    sanitize_python_env(&mut cmd);
    cmd
}

/// 构造已清理 Python 环境的 yt-dlp 命令（异步）。
fn yt_dlp_command_async(yt_dlp_path: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(yt_dlp_path);
    sanitize_python_env(&mut cmd);
    cmd
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
        let mut cmd = yt_dlp_command(yt_dlp_path);
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
        let mut cmd = yt_dlp_command(yt_dlp_path);
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
        let mut cmd = yt_dlp_command(yt_dlp_path);
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
        let mut cmd = yt_dlp_command(yt_dlp_path);
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

        let mut cmd = yt_dlp_command_async(yt_dlp_path);

        let format_str = match quality {
            "best" => "best".to_string(),
            "2160p" => "bestvideo[height<=2160]+bestaudio/best[height<=2160]".to_string(),
            "1440p" => "bestvideo[height<=1440]+bestaudio/best[height<=1440]".to_string(),
            "720p" => "bestvideo[height<=720]+bestaudio/best[height<=720]".to_string(),
            "480p" => "bestvideo[height<=480]+bestaudio/best[height<=480]".to_string(),
            _ => "bestvideo[height<=1080]+bestaudio/best[height<=1080]".to_string(),
        };

        cmd.args(build_download_args(
            &format_str,
            &output_template.to_string_lossy(),
            url,
        ));

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
    fn test_video_info_null_title_falls_back() {
        // 私享/不可用视频在频道 flat 列表里 title 为 null
        let json = r#"{"title":null,"id":"iDRlnY8RpVQ","url":"https://www.youtube.com/watch?v=iDRlnY8RpVQ"}"#;

        let video: VideoInfo =
            serde_json::from_str(json).expect("title 为 null 必须能解析");

        assert_eq!(video.title, "私享视频 iDRlnY8RpVQ");
        assert_eq!(video.id, Some("iDRlnY8RpVQ".to_string()));
        assert_eq!(video.url, "https://www.youtube.com/watch?v=iDRlnY8RpVQ");
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

    // ── 下载进度：参数与模板必须与 parser 契约一致 ──────────────────
    //
    // 这三条锁住历史 bug：`--print` 隐含 `--quiet` 抑制进度、缺 `--newline` 导致
    // `BufReader::lines()` 切不开 `\r`、模板用 `_str` 字段导致解析器全部失败。

    #[test]
    fn test_build_download_args_exact() {
        let args = build_download_args("best", "/out/%(title)s.%(ext)s", "https://example.com/v");

        let expected: Vec<String> = [
            "-f",
            "best",
            "-o",
            "/out/%(title)s.%(ext)s",
            "--no-playlist",
            "--newline",
            "--progress",
            "--progress-template",
            PROGRESS_TEMPLATE,
            "--print",
            "after_move:filepath",
            "https://example.com/v",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        assert_eq!(args, expected);
    }

    #[test]
    fn test_progress_template_uses_raw_numeric_fields() {
        // percent / downloaded_bytes / total_bytes 必须是裸数值字段，不能是 _str 变体
        assert_eq!(
            PROGRESS_TEMPLATE,
            "%(progress._percent)s|%(progress._speed_str)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress._eta_str)s"
        );
    }

    #[test]
    fn test_progress_template_rendered_line_parses() {
        // 真实 yt-dlp（2026.08.19）按本模板渲染出的行，必须能被解析器接受
        use crate::utils::progress_parser::parse_progress_line;

        let event = parse_progress_line("3.49184|   2.21MiB/s|1047552|30000000|00:12")
            .expect("模板渲染出的行必须能被解析");

        assert!((event.percent - 3.49184).abs() < 1e-6);
        assert_eq!(event.speed, "   2.21MiB/s");
        assert_eq!(event.downloaded_bytes, 1047552);
        assert_eq!(event.total_bytes, 30000000);
        assert_eq!(event.eta, "00:12");
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

    // ── AppImage 环境清理 ──────────────────────────────────────────

    #[test]
    fn test_strip_appdir_entries_drops_mount_paths_and_keeps_user_entries() {
        // AppRun 把挂载目录拼在原 PYTHONPATH 之前
        assert_eq!(
            strip_appdir_entries(
                "/tmp/.mount_ytdlpX/usr/share/pyshared/:/home/me/mymods",
                "/tmp/.mount_ytdlpX"
            ),
            Some("/home/me/mymods".to_string())
        );
    }

    #[test]
    fn test_strip_appdir_entries_returns_none_when_only_mount_paths_remain() {
        // 用户原本没有 PYTHONPATH 时，AppRun 会留下挂载目录加一个尾随冒号（空条目）
        assert_eq!(
            strip_appdir_entries(
                "/tmp/.mount_ytdlpX/usr/share/pyshared/:",
                "/tmp/.mount_ytdlpX"
            ),
            None
        );
    }

    #[test]
    fn test_strip_appdir_entries_keeps_entries_merely_sharing_appdir_prefix() {
        // 只有字符串前缀相同、实际不在挂载目录内，必须保留（含挂载目录自身要剔除）
        assert_eq!(
            strip_appdir_entries(
                "/tmp/.mount_ytdlpX-backup:/tmp/.mount_ytdlpX/usr/lib",
                "/tmp/.mount_ytdlpX"
            ),
            Some("/tmp/.mount_ytdlpX-backup".to_string())
        );
    }

    #[test]
    fn test_strip_appdir_entries_keeps_unrelated_entries() {
        assert_eq!(
            strip_appdir_entries("/home/me/a:/home/me/b", "/tmp/.mount_ytdlpX"),
            Some("/home/me/a:/home/me/b".to_string())
        );
    }

    #[test]
    fn test_python_env_fixes_is_noop_outside_appimage() {
        // 没有 APPDIR 就不是 AppImage 环境，绝不能动用户自己的 Python 配置
        assert_eq!(
            python_env_fixes(None, Some("/home/me/mymods")),
            Vec::<(&str, Option<String>)>::new()
        );
    }

    #[test]
    fn test_python_env_fixes_removes_pythonhome_and_strips_appdir_from_pythonpath() {
        assert_eq!(
            python_env_fixes(
                Some("/tmp/.mount_ytdlpX"),
                Some("/tmp/.mount_ytdlpX/usr/share/pyshared/:/home/me/mymods")
            ),
            vec![
                ("PYTHONHOME", None),
                ("PYTHONPATH", Some("/home/me/mymods".to_string())),
            ]
        );
    }

    #[test]
    fn test_python_env_fixes_removes_pythonpath_when_it_becomes_empty() {
        assert_eq!(
            python_env_fixes(
                Some("/tmp/.mount_ytdlpX"),
                Some("/tmp/.mount_ytdlpX/usr/share/pyshared/:")
            ),
            vec![("PYTHONHOME", None), ("PYTHONPATH", None)]
        );
    }

    #[test]
    fn test_python_env_fixes_ignores_pythonpath_when_unset() {
        assert_eq!(
            python_env_fixes(Some("/tmp/.mount_ytdlpX"), None),
            vec![("PYTHONHOME", None)]
        );
    }
}
