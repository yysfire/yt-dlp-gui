use serde::{Deserialize, Serialize};

/// 单个视频信息（详情面板分页列表条目）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    /// 平台视频 ID（如 YouTube video ID）
    pub id: String,
    /// 视频标题
    pub title: String,
    /// 视频 URL
    pub url: String,
    /// 时长（如 "12:34"）
    pub duration: Option<String>,
    /// 上传日期（YYYYMMDD 格式）
    pub upload_date: Option<String>,
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
