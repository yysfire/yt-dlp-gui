/** Represents a channel subscription. */
export interface Subscription {
  id: string;
  url: string;
  platform: "youtube" | "bilibili" | "other";
  channel_name: string;
  channel_avatar_url: string;
  paused: boolean;
  quality_preset: string;
  created_at: string; // ISO 8601
}

/** Represents a single video download record. */
export interface DownloadRecord {
  id: string;
  subscription_id: string;
  video_title: string;
  video_url: string;
  file_path: string;
  file_size: number;
  status: "downloading" | "completed" | "failed";
  downloaded_at: string; // ISO 8601
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
