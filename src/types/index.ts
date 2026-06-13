/** 订阅健康状态 */
export type HealthStatus = "ok" | "warning" | "dead";

/** 单个订阅的健康检查结果（运行时用） */
export interface HealthCheckResult {
  subscription_id: string;
  url: string;
  status: HealthStatus;
  detail: string;
  latency_ms: number;
  checked_at: string;
}

/** 健康检查摘要 */
export interface HealthCheckSummary {
  total: number;
  ok: number;
  warning: number;
  dead: number;
  duration_ms: number;
  results: HealthCheckResult[];
}

/** Represents a channel subscription. */
export interface Subscription {
  id: string;
  url: string;
  platform: "youtube" | "bilibili" | "other";
  channel_name: string;
  channel_avatar_url: string;
  paused: boolean;
  quality_preset: string;
  group_name: string;
  created_at: string; // ISO 8601
  last_checked_at: string | null;
  download_count: number;
  last_check_status: "success" | "failed" | null;
  last_check_error: string | null;
  /** 分组标签名数组（如 ["学习", "音乐"]） */
  tags: string[];
  /** 健康状态，null 表示未检查 */
  health_status: HealthStatus | null;
  /** 上次健康检查时间（ISO 8601） */
  last_health_check: string | null;
}

/** Represents a single video download record. */
export interface DownloadRecord {
  id: string;
  subscription_id: string;
  video_id: string;
  video_title: string;
  video_url: string;
  file_path: string;
  file_size: number;
  status: "downloading" | "completed" | "failed" | "paused" | "cancelled" | "waiting" | "deleted";
  error_message: string | null;
  downloaded_at: string; // ISO 8601
}

/** Result of checking whether a file exists on disk. */
export interface FileExistenceResult {
  file_path: string;
  exists: boolean;
}

/** Download task status in memory queue */
export type TaskStatus = "waiting" | "running" | "paused" | "completed" | "failed" | "cancelled";

/** Real-time download progress */
export interface DownloadProgress {
  task_id: string;
  video_url: string;
  percent: number;
  speed: string;
  downloaded_bytes: number;
  total_bytes: number;
  eta: string;
}

/** Download task in the in-memory queue */
export interface DownloadTask {
  id: string;
  video_id: string;
  video_url: string;
  video_title: string;
  subscription_id: string;
  quality: string;
  status: TaskStatus;
  progress: DownloadProgress | null;
  error_message: string | null;
  created_at: string;
  completed_at: string | null;
}

/** Download queue runtime state */
export interface QueueState {
  active_count: number;
  waiting_count: number;
  max_concurrent: number;
}

/** Application-wide settings. */
export interface AppSettings {
  download_dir: string;
  check_interval_minutes: number;
  yt_dlp_path: string;
  quality_preset: string;
  notifications_enabled: boolean;
  dark_mode: boolean;
  proxy_url: string;
  cookie_file: string;
  max_concurrent_downloads: number;
  /** When true, window minimizes to system tray instead of taskbar (default: true) */
  minimize_to_tray: boolean;
  /** When true, closing the window hides to tray instead of exiting (default: true) */
  close_to_tray: boolean;
  /** When true, application starts minimized to system tray (default: false) */
  start_in_tray: boolean;
  /** When true, the background scheduler is paused (default: false) */
  scheduler_paused: boolean;
}

/** Runtime system tray state. */
export interface TrayState {
  status: "idle" | "downloading" | "checking";
  window_visible: boolean;
  tray_supported: boolean;
  active_downloads: number;
}

/** Result of download path validation. */
export interface PathValidateResult {
  valid: boolean;
  writable: boolean;
  exists: boolean;
  error: string | null;
}

/** Result of proxy URL validation. */
export interface ProxyValidateResult {
  valid: boolean;
  scheme: string | null;
  error: string | null;
}

/** Result of a batch import operation. */
export interface ImportResult {
  imported: Subscription[];
  skipped_duplicates: string[];
  skipped_invalid: string[];
  total: number;
  success_count: number;
}

/** 导入源类型 */
export type ImportSourceType = "opml" | "txt" | "url_list";

/** 导入源（区分 OPML 文件、TXT 文件、URL 列表） */
export type ImportSource =
  | { type: "opml"; path: string }
  | { type: "txt"; path: string }
  | { type: "url_list"; urls: string[] };

/** 导入进度事件负载 */
export interface ImportProgressEvent {
  task_id: string;
  completed: number;
  total: number;
  current_url: string;
  current_title: string;
}

/** 导入完成事件负载 */
export interface ImportCompleteEvent {
  task_id: string;
  total: number;
  success: number;
  failed: number;
  skipped: number;
  errors: ImportErrorItem[];
}

/** 导入错误项 */
export interface ImportErrorItem {
  url: string;
  reason: string;
}

/** 导入预览项 */
export interface ImportPreviewItem {
  url: string;
  title: string | null;
  is_duplicate: boolean;
  error: string | null;
}

/** 导入预览结果 */
export interface ImportPreview {
  total: number;
  items: ImportPreviewItem[];
  duplicates: number;
}

/** 频道信息 */
export interface ChannelInfo {
  channel_name: string;
  description: string | null;
  subscriber_count: string | null;
  video_count: number | null;
  thumbnail_url: string | null;
  last_refreshed: string;
}

/** 视频信息 */
export interface VideoInfo {
  id: string;
  title: string;
  url: string;
  duration: number | null;
  upload_date: string | null;
  thumbnail: string | null;
}

/** 统一视频条目状态 */
export type VideoStatus =
  | "new"
  | "downloading"
  | "paused"
  | "waiting"
  | "completed"
  | "failed"
  | "cancelled";

/** 统一视频列表条目：合并频道视频与下载记录 */
export interface UnifiedVideoItem {
  /** 频道视频信息（仅当该视频来自频道播放列表时有值） */
  channelInfo?: VideoInfo;
  /** 持久化的下载记录（仅当该视频已被下载/下载中/失败时有值） */
  downloadInfo?: DownloadRecord;
  /** 内存中的下载队列任务（仅当该视频在活跃队列中时有值） */
  queueTask?: DownloadTask;
  /** 唯一标识：优先 video_id，回退 video_url */
  readonly id: string;
  /** 显示标题：优先频道视频标题，回退下载记录标题 */
  readonly title: string;
  /** 视频链接 */
  readonly url: string;
  /** 视频当前状态 */
  readonly status: VideoStatus;
}

/** 视频列表分页结果 */
export interface VideoListResult {
  videos: VideoInfo[];
  total: number;
  page: number;
  page_size: number;
  has_more: boolean;
}

/** Runtime application state. */
export interface AppState {
  last_check_time: string | null;
  total_downloads: number;
}

/** Predefined subscription groups. */
export const GROUPS = ["未分组", "学习", "娱乐", "音乐", "科技", "其他"] as const;
export type GroupName = typeof GROUPS[number];

/** 筛选条件（会话内持久化） */
export interface FilterState {
  platform: "all" | "youtube" | "bilibili";
  status: "all" | "active" | "paused";
  group: string;
  health: "all" | "ok" | "warning" | "dead" | "unchecked";
  keyword: string;
}

/** 排序配置 */
export interface SortState {
  field: "name" | "created_at" | "last_health_check";
  direction: "asc" | "desc";
}
