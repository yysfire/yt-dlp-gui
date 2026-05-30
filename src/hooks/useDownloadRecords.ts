import { useState, useEffect, useCallback } from "react";
import type { DownloadRecord } from "@/types";
import * as api from "@/lib/tauri";

interface UseDownloadRecordsReturn {
  records: DownloadRecord[];
  loading: boolean;
  error: string | null;
  refresh: (subscriptionId?: string) => Promise<void>;
  checkSubscription: (id: string) => Promise<void>;
  checkAll: () => Promise<void>;
  getQueueState: () => Promise<import("@/types").QueueState | null>;
  getQueue: () => Promise<import("@/types").DownloadTask[]>;
}

/**
 * Hook for managing download records state.
 * Supports filtering by subscription ID and triggering checks.
 */
export function useDownloadRecords(): UseDownloadRecordsReturn {
  const [records, setRecords] = useState<DownloadRecord[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async (subscriptionId?: string) => {
    try {
      setError(null);
      const data = subscriptionId
        ? await api.getDownloadRecords(subscriptionId)
        : await api.getAllDownloadRecords();
      setRecords(data);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const checkSubscription = useCallback(async (id: string) => {
    try {
      setError(null);
      setLoading(true);
      const newRecords = await api.checkSubscription(id);
      setRecords((prev) => {
        // Replace existing records with updated ones, add new ones
        const map = new Map(prev.map((r) => [r.id, r]));
        for (const nr of newRecords) {
          map.set(nr.id, nr);
        }
        return Array.from(map.values());
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const checkAll = useCallback(async () => {
    try {
      setError(null);
      setLoading(true);
      const newRecords = await api.checkAllSubscriptions();
      setRecords((prev) => {
        const map = new Map(prev.map((r) => [r.id, r]));
        for (const nr of newRecords) {
          map.set(nr.id, nr);
        }
        return Array.from(map.values());
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    setLoading(true);
    refresh().finally(() => setLoading(false));
  }, [refresh]);

  const getQueueState = useCallback(async () => {
    try {
      return await api.getQueueState();
    } catch (e) {
      setError(String(e));
      return null;
    }
  }, []);

  const getQueue = useCallback(async () => {
    try {
      return await api.getDownloadQueue();
    } catch (e) {
      setError(String(e));
      return [];
    }
  }, []);

  return { records, loading, error, refresh, checkSubscription, checkAll, getQueueState, getQueue };
}
