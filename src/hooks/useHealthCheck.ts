import { useState, useCallback, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { HealthCheckSummary } from "@/types";
import * as api from "@/lib/tauri";

interface HealthCheckProgress {
  completed: number;
  total: number;
  current_url: string;
}

interface UseHealthCheckReturn {
  isChecking: boolean;
  progress: { completed: number; total: number } | null;
  summary: HealthCheckSummary | null;
  startCheckAll: () => Promise<void>;
  startCheckSelected: (ids: string[]) => Promise<void>;
  clearResults: () => void;
}

/**
 * Hook for managing subscription health check state.
 * Listens for health-check-progress and health-check-complete events from the backend.
 */
export function useHealthCheck(
  refreshSubscriptions: () => void,
): UseHealthCheckReturn {
  const [isChecking, setIsChecking] = useState(false);
  const [progress, setProgress] = useState<{
    completed: number;
    total: number;
  } | null>(null);
  const [summary, setSummary] = useState<HealthCheckSummary | null>(null);

  // Listen for progress events
  useEffect(() => {
    const unlistenPromise = listen<HealthCheckProgress>(
      "health-check-progress",
      (event) => {
        setProgress({
          completed: event.payload.completed,
          total: event.payload.total,
        });
      },
    );
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  // Listen for completion events
  useEffect(() => {
    const unlistenPromise = listen<HealthCheckSummary>(
      "health-check-complete",
      (event) => {
        setSummary(event.payload);
        setIsChecking(false);
        setProgress(null);
        refreshSubscriptions();
      },
    );
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [refreshSubscriptions]);

  const startCheckAll = useCallback(async () => {
    setIsChecking(true);
    setProgress({ completed: 0, total: 0 });
    setSummary(null);
    try {
      await api.checkAllHealth();
    } catch (e) {
      setIsChecking(false);
      setProgress(null);
      console.error("健康检查失败:", e);
    }
  }, []);

  const startCheckSelected = useCallback(async (ids: string[]) => {
    if (ids.length === 0) return;
    setIsChecking(true);
    setProgress({ completed: 0, total: 0 });
    setSummary(null);
    try {
      await api.checkSelectedHealth(ids);
    } catch (e) {
      setIsChecking(false);
      setProgress(null);
      console.error("选择性健康检查失败:", e);
    }
  }, []);

  const clearResults = useCallback(() => {
    setSummary(null);
  }, []);

  return {
    isChecking,
    progress,
    summary,
    startCheckAll,
    startCheckSelected,
    clearResults,
  };
}
