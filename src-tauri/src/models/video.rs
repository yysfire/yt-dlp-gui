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

/// 单个视频信息（详情面板分页列表条目）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    /// 平台视频 ID（如 YouTube video ID）
    pub id: String,
    /// 视频标题
    pub title: String,
    /// 视频 URL
    pub url: String,
    /// 时长（秒，浮点数；yt-dlp --flat-playlist 输出为浮点类型）
    #[serde(default, deserialize_with = "deserialize_duration")]
    pub duration: Option<f64>,
    /// 上传日期（YYYYMMDD 格式；yt-dlp --flat-playlist 不输出此字段）
    #[serde(default)]
    pub upload_date: Option<String>,
    /// Unix 时间戳（秒）；yt-dlp --flat-playlist 输出 epoch 字段作为回退
    #[serde(default)]
    pub epoch: Option<i64>,
    /// 缩略图 URL
    pub thumbnail: Option<String>,
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
