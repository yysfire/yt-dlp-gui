import { invoke } from "@tauri-apps/api/core";
import type {
  Subscription,
  DownloadRecord,
  DownloadTask,
  QueueState,
  AppSettings,
  AppState,
  ImportPreview,
  ImportResult,
  ImportSource,
  PathValidateResult,
  ProxyValidateResult,
  ChannelInfo,
  VideoListResult,
} from "@/types";

// ── Subscription commands ──────────────────────────────────────────

/** Adds a new subscription by parsing the channel URL. */
export async function addSubscription(url: string): Promise<Subscription> {
  return invoke<Subscription>("add_subscription", { url });
}

/** Deletes a subscription and its associated download records. */
export async function deleteSubscription(id: string): Promise<void> {
  return invoke<void>("delete_subscription", { id });
}

/** Returns all subscriptions. */
export async function getSubscriptions(): Promise<Subscription[]> {
  return invoke<Subscription[]>("get_subscriptions");
}

/** Toggles the paused state of a subscription. */
export async function toggleSubscriptionPause(
  id: string,
): Promise<Subscription> {
  return invoke<Subscription>("toggle_subscription_pause", { id });
}

/** Updates the quality preset for a subscription. */
export async function updateSubscriptionQuality(
  id: string,
  qualityPreset: string,
): Promise<Subscription> {
  return invoke<Subscription>("update_subscription_quality", {
    id,
    qualityPreset,
  });
}

/** Updates the group for a subscription. */
export async function updateSubscriptionGroup(
  id: string,
  groupName: string,
): Promise<Subscription> {
  return invoke<Subscription>("update_subscription_group", {
    id,
    groupName,
  });
}

// ── Download commands ──────────────────────────────────────────────

/** Checks a single subscription for new videos. */
export async function checkSubscription(
  id: string,
): Promise<DownloadRecord[]> {
  return invoke<DownloadRecord[]>("check_subscription", { id });
}

/** Checks all subscriptions for new videos. */
export async function checkAllSubscriptions(): Promise<DownloadRecord[]> {
  return invoke<DownloadRecord[]>("check_all_subscriptions");
}

/** Returns download records, optionally filtered by subscription ID. */
export async function getDownloadRecords(
  subscriptionId?: string,
): Promise<DownloadRecord[]> {
  return invoke<DownloadRecord[]>("get_download_records", {
    subscription_id: subscriptionId ?? null,
  });
}

/** Returns all download records. */
export async function getAllDownloadRecords(): Promise<DownloadRecord[]> {
  return invoke<DownloadRecord[]>("get_all_download_records");
}

/** Manually triggers a full check of all subscriptions. */
export async function manualCheckAll(): Promise<DownloadRecord[]> {
  return invoke<DownloadRecord[]>("manual_check_all");
}

/** Pauses an active download task. */
export async function pauseDownload(id: string): Promise<void> {
  return invoke<void>("pause_download", { id });
}

/** Resumes a paused download task. */
export async function resumeDownload(id: string): Promise<void> {
  return invoke<void>("resume_download", { id });
}

/** Cancels a download task and cleans up partial files. */
export async function cancelDownload(id: string): Promise<void> {
  return invoke<void>("cancel_download", { id });
}

/** Pauses an active download identified by video URL. */
export async function pauseDownloadByUrl(videoUrl: string): Promise<void> {
  return invoke<void>("pause_download_by_url", { videoUrl });
}

/** Cancels a download identified by video URL. */
export async function cancelDownloadByUrl(videoUrl: string): Promise<void> {
  return invoke<void>("cancel_download_by_url", { videoUrl });
}

/** Returns the current in-memory download queue. */
export async function getDownloadQueue(): Promise<DownloadTask[]> {
  return invoke<DownloadTask[]>("get_download_queue");
}

/** Returns the runtime download queue state. */
export async function getQueueState(): Promise<QueueState> {
  return invoke<QueueState>("get_queue_state");
}

// ── Settings commands ──────────────────────────────────────────────

/** Returns the current application settings. */
export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

/** Updates application settings. */
export async function updateSettings(
  settings: AppSettings,
): Promise<AppSettings> {
  return invoke<AppSettings>("update_settings", { settings });
}

/** Returns the current application state. */
export async function getAppState(): Promise<AppState> {
  return invoke<AppState>("get_app_state");
}

/** Starts the background scheduler. */
export async function startScheduler(): Promise<void> {
  return invoke<void>("start_scheduler");
}

/** Stops the background scheduler. */
export async function stopScheduler(): Promise<void> {
  return invoke<void>("stop_scheduler");
}

/** Validates a download directory path for existence and writability. */
export async function validateDownloadPath(
  path: string,
): Promise<PathValidateResult> {
  return invoke<PathValidateResult>("validate_download_path", { path });
}

/** Validates a proxy URL format (supports http, https, socks5, socks5h). */
export async function validateProxyUrl(
  url: string,
): Promise<ProxyValidateResult> {
  return invoke<ProxyValidateResult>("validate_proxy_url", { url });
}

// ── File management commands ─────────────────────────────────────

/** Result of checking whether a file exists on disk. */
export interface FileExistenceResult {
  file_path: string;
  exists: boolean;
}

/** Opens the parent directory of a file in the system file manager. */
export async function openInFolder(filePath: string): Promise<void> {
  return invoke<void>("open_in_folder", { filePath });
}

/** Batch-checks whether files exist on disk. */
export async function checkFileExistence(
  filePaths: string[],
): Promise<FileExistenceResult[]> {
  return invoke<FileExistenceResult[]>("check_file_existence", { filePaths });
}

/** Deletes a downloaded video file and marks the record as "deleted". */
export async function deleteFile(id: string): Promise<void> {
  return invoke<void>("delete_file", { id });
}

/** Triggers a file state sync for all completed download records. */
export async function syncFileStates(): Promise<FileExistenceResult[]> {
  return invoke<FileExistenceResult[]>("sync_file_states");
}

/** Exports all subscriptions to a JSON file at the given path. */
export async function exportSubscriptionsJson(path: string): Promise<void> {
  return invoke<void>("export_subscriptions_json", { path });
}

/** Exports all subscriptions to an OPML file at the given path. */
export async function exportSubscriptionsOpml(path: string): Promise<void> {
  return invoke<void>("export_subscriptions_opml", { path });
}

// ── Tray state ──────────────────────────────────────────────────

/** Re-export TrayState for frontend event listeners. */
export type { TrayState } from "@/types";

/**
 * Batch imports subscriptions from URLs and/or a file path.
 *
 * @param urls - List of channel URLs (used in paste mode).
 * @param filePath - Path to a .txt or .opml file (used in file mode).
 */
export async function batchImportSubscriptions(
  urls: string[],
  filePath?: string,
): Promise<ImportResult> {
  return invoke<ImportResult>("batch_import_subscriptions", {
    urls,
    filePath: filePath ?? null,
  });
}

// ── Enhanced import commands ────────────────────────────────────

/** Re-export ImportSource for convenience. */
export type { ImportSource, ImportProgressEvent, ImportCompleteEvent } from "@/types";

/** Parses an import source and returns a preview without creating subscriptions. */
export async function batchImportPreview(
  source: ImportSource,
): Promise<ImportPreview> {
  return invoke<ImportPreview>("batch_import_preview", { source });
}

/** Executes the batch import as a background task. Returns the task_id for tracking. */
export async function batchImportExecute(
  source: ImportSource,
  skipDuplicates: boolean,
): Promise<string> {
  return invoke<string>("batch_import_execute", { source, skipDuplicates });
}

/** Cancels a running batch import task. No-op if the task is not running. */
export async function cancelImport(taskId: string): Promise<void> {
  return invoke<void>("cancel_import", { taskId });
}

// ── Health check commands ─────────────────────────────────────────

/** Runs a health check on all subscriptions. */
export async function checkAllHealth(): Promise<{
  started: boolean;
  total: number;
}> {
  return invoke<{ started: boolean; total: number }>("check_all_health");
}

/** Runs a health check on selected subscriptions. */
export async function checkSelectedHealth(
  subscriptionIds: string[],
): Promise<{ started: boolean; total: number }> {
  return invoke<{ started: boolean; total: number }>("check_selected_health", {
    subscriptionIds,
  });
}

/** Batch deletes subscriptions by IDs. */
export async function batchDeleteSubscriptions(
  ids: string[],
): Promise<{ deleted_count: number }> {
  return invoke<{ deleted_count: number }>("batch_delete_subscriptions", {
    ids,
  });
}

// ── Channel info & video list commands ─────────────────────────────

/** 获取订阅频道的详细信息（名称、描述、订阅数、视频数等）。 */
export async function getChannelInfo(
  subscriptionId: string,
): Promise<ChannelInfo> {
  return invoke<ChannelInfo>("get_channel_info", { subscriptionId });
}

/** 分页获取订阅频道的最新视频列表。 */
export async function getChannelVideos(
  subscriptionId: string,
  page: number = 1,
  pageSize: number = 10,
): Promise<VideoListResult> {
  return invoke<VideoListResult>("get_channel_videos", {
    subscriptionId,
    page,
    pageSize,
  });
}
