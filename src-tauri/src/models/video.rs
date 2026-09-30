use serde::{Deserialize, Deserializer, Serialize};

/// 自定义反序列化器：duration 字段在 yt-dlp 输出中可能是浮点数或是字符串
fn deserialize_duration<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DurationValue {
        Float(f64),
        Int(i64),
        String(String),
        Null,
    }

    match Option::<DurationValue>::deserialize(deserializer)? {
        None | Some(DurationValue::Null) => Ok(None),
        Some(DurationValue::Float(f)) => Ok(Some(f)),
        Some(DurationValue::Int(i)) => Ok(Some(i as f64)),
        Some(DurationValue::String(s)) => {
            // 尝试直接解析为浮点数
            if let Ok(f) = s.parse::<f64>() {
                return Ok(Some(f));
            }
            // 尝试解析时间格式 "h:mm:ss" 或 "mm:ss"
            let parts: Vec<&str> = s.split(':').collect();
            match parts.len() {
                2 => {
                    let m: f64 = parts[0].parse().map_err(serde::de::Error::custom)?;
                    let s: f64 = parts[1].parse().map_err(serde::de::Error::custom)?;
                    Ok(Some(m * 60.0 + s))
                }
                3 => {
                    let h: f64 = parts[0].parse().map_err(serde::de::Error::custom)?;
                    let m: f64 = parts[1].parse().map_err(serde::de::Error::custom)?;
                    let s: f64 = parts[2].parse().map_err(serde::de::Error::custom)?;
                    Ok(Some(h * 3600.0 + m * 60.0 + s))
                }
                _ => Err(serde::de::Error::custom(format!(
                    "invalid duration format: {}",
                    s
                ))),
            }
        }
    }
}

/// 统一 yt-dlp 输出中视频 URL 的两种键名。
///
/// yt-dlp 的键名并不统一：`--flat-playlist` 的条目**同时**给出 `url` 与 `webpage_url`
/// （取值相同），而单个视频 URL（不是频道/播放列表）走的是完整元数据，只有 `webpage_url`。
/// 正因为两种键会同时出现，**不能**给 `url` 加 `#[serde(alias = "webpage_url")]` —— serde
/// 会把第二个键当成重复字段并直接报 `duplicate field` 错误。故统一在此处解析：
/// 优先 `url`，回退 `webpage_url`，两者都缺才报错。
pub(crate) fn resolve_video_url(
    url: Option<String>,
    webpage_url: Option<String>,
) -> Result<String, String> {
    url.or(webpage_url)
        .ok_or_else(|| "missing field `url`".to_string())
}

/// 单个视频信息（详情面板分页列表条目）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawVideoInfo")]
pub struct VideoInfo {
    /// 平台视频 ID（如 YouTube video ID）
    pub id: String,
    /// 视频标题
    pub title: String,
    /// 视频 URL（由 `url` 或 `webpage_url` 统一而来，见 `resolve_video_url`）
    pub url: String,
    /// 时长（秒，浮点数；yt-dlp --flat-playlist 输出为浮点类型）
    pub duration: Option<f64>,
    /// 上传日期（YYYYMMDD 格式；yt-dlp --flat-playlist 不输出此字段）
    pub upload_date: Option<String>,
    /// Unix 时间戳（秒）；yt-dlp --flat-playlist 输出 epoch 字段作为回退
    pub epoch: Option<i64>,
    /// 缩略图 URL
    pub thumbnail: Option<String>,
}

/// yt-dlp 输出行的原始字段，专供 `VideoInfo` 反序列化。
///
/// 与 `VideoInfo` 的唯一差别是 URL 可能落在 `url` 或 `webpage_url` 上，
/// 构造 `VideoInfo` 时由 `resolve_video_url` 归一。
#[derive(Deserialize)]
struct RawVideoInfo {
    id: String,
    title: String,
    url: Option<String>,
    webpage_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_duration")]
    duration: Option<f64>,
    #[serde(default)]
    upload_date: Option<String>,
    #[serde(default)]
    epoch: Option<i64>,
    thumbnail: Option<String>,
}

impl TryFrom<RawVideoInfo> for VideoInfo {
    type Error = String;

    fn try_from(raw: RawVideoInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            id: raw.id,
            title: raw.title,
            url: resolve_video_url(raw.url, raw.webpage_url)?,
            duration: raw.duration,
            upload_date: raw.upload_date,
            epoch: raw.epoch,
            thumbnail: raw.thumbnail,
        })
    }
}

/// 视频列表分页结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoListResult {
    /// 当前页视频列表
    pub videos: Vec<VideoInfo>,
    /// 频道总视频数
    pub total: usize,
    /// 当前页码（1-based）
    pub page: u32,
    /// 每页条数
    pub page_size: u32,
    /// 是否还有更多页
    pub has_more: bool,
}

/// 频道详细信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// 频道名称
    pub channel_name: String,
    /// 频道描述
    pub description: Option<String>,
    /// 订阅者数（格式化字符串，如 "12.3K"）
    pub subscriber_count: Option<String>,
    /// 总视频数
    pub video_count: Option<u64>,
    /// 频道缩略图 URL
    pub thumbnail_url: Option<String>,
    /// 信息最后刷新时间（ISO 8601）
    pub last_refreshed: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_video_url_prefers_url() {
        let resolved = resolve_video_url(
            Some("https://example.com/from-url".to_string()),
            Some("https://example.com/from-webpage-url".to_string()),
        );
        assert_eq!(resolved, Ok("https://example.com/from-url".to_string()));
    }

    #[test]
    fn test_resolve_video_url_errors_when_both_missing() {
        let resolved = resolve_video_url(None, None);
        assert_eq!(resolved, Err("missing field `url`".to_string()));
    }

    #[test]
    fn test_video_info_parses_channel_flat_entry_with_both_url_keys() {
        // 真实频道 flat 条目：url 与 webpage_url 同时存在且取值相同。
        // 这里曾尝试给 url 加 #[serde(alias = "webpage_url")]，会因重复字段直接失败。
        let json = r#"{"_type":"url","id":"llHsd-dI50E","title":"Test Video","url":"https://www.youtube.com/watch?v=llHsd-dI50E","webpage_url":"https://www.youtube.com/watch?v=llHsd-dI50E","duration":71,"epoch":1790779395}"#;

        let video: VideoInfo =
            serde_json::from_str(json).expect("频道 flat 条目应能解析");

        assert_eq!(video.id, "llHsd-dI50E");
        assert_eq!(video.url, "https://www.youtube.com/watch?v=llHsd-dI50E");
        assert_eq!(video.duration, Some(71.0));
        assert_eq!(video.epoch, Some(1790779395));
    }

    #[test]
    fn test_video_info_parses_single_video_full_metadata() {
        // 单视频 URL（非频道/播放列表）的完整元数据：没有顶层 url，只有 webpage_url
        let json = r#"{"id":"lcH2wJMVmP4","title":"Single Video","webpage_url":"https://www.youtube.com/watch?v=lcH2wJMVmP4","original_url":"https://www.youtube.com/watch?v=lcH2wJMVmP4","duration":33,"upload_date":"20260922","epoch":1790779138,"thumbnail":"https://i.ytimg.com/vi/lcH2wJMVmP4/maxresdefault.jpg"}"#;

        let video: VideoInfo =
            serde_json::from_str(json).expect("单视频完整元数据应能解析");

        assert_eq!(video.id, "lcH2wJMVmP4");
        assert_eq!(video.url, "https://www.youtube.com/watch?v=lcH2wJMVmP4");
        assert_eq!(video.duration, Some(33.0));
        assert_eq!(video.upload_date, Some("20260922".to_string()));
        assert_eq!(video.epoch, Some(1790779138));
        assert_eq!(
            video.thumbnail,
            Some("https://i.ytimg.com/vi/lcH2wJMVmP4/maxresdefault.jpg".to_string())
        );
    }

    #[test]
    fn test_video_info_missing_url_is_error() {
        // 两种键都缺时必须报错，不能产生 url 为空的视频
        let err = serde_json::from_str::<VideoInfo>(r#"{"id":"x","title":"t"}"#)
            .expect_err("缺少 url 时应报错");

        assert_eq!(err.to_string(), "missing field `url`");
    }
}
