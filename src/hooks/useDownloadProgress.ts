import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import type { DownloadProgress } from "@/types";

interface UseDownloadProgressReturn {
  /** Map of video_url to progress */
  progressMap: Map<string, DownloadProgress>;
  /** Get progress for a specific video_url */
  getProgress: (videoUrl: string) => DownloadProgress | null;
}

/**
 * Hook for listening to real-time download progress events.
 * Indexes progress by video_url for easy matching with DownloadRecord.
 */
export function useDownloadProgress(): UseDownloadProgressReturn {
  const [progressMap, setProgressMap] = useState<Map<string, DownloadProgress>>(
    new Map(),
  );

  const getProgress = useCallback(
    (videoUrl: string): DownloadProgress | null => {
      return progressMap.get(videoUrl) ?? null;
    },
    [progressMap],
  );

  useEffect(() => {
    const unlistenPromise = listen<DownloadProgress>(
      "download-progress",
      (event) => {
        const progress = event.payload;
        setProgressMap((prev) => {
          const next = new Map(prev);
          next.set(progress.video_url, progress);
          return next;
        });
      },
    );

    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  return { progressMap, getProgress };
}
