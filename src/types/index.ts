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
  status: "downloading" | "completed" | "failed" | "paused" | "cancelled";
  error_message: string | null;
  downloaded_at: string; // ISO 8601
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
}

/** Result of a batch import operation. */
export interface ImportResult {
  imported: Subscription[];
  skipped_duplicates: string[];
  skipped_invalid: string[];
  total: number;
  success_count: number;
}

/** Runtime application state. */
export interface AppState {
  last_check_time: string | null;
  total_downloads: number;
}

/** Predefined subscription groups. */
export const GROUPS = ["未分组", "学习", "娱乐", "音乐", "科技", "其他"] as const;
export type GroupName = typeof GROUPS[number];
