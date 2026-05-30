import { useState, useEffect, useCallback } from "react";
import type { Subscription } from "@/types";
import * as api from "@/lib/tauri";

interface UseSubscriptionsReturn {
  subscriptions: Subscription[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  addSubscription: (url: string) => Promise<Subscription>;
  deleteSubscription: (id: string) => Promise<void>;
  togglePause: (id: string) => Promise<void>;
  updateQuality: (id: string, quality: string) => Promise<void>;
  updateGroup: (id: string, groupName: string) => Promise<void>;
}

/**
 * Hook for managing the subscription list state.
 * Handles loading, error, and CRUD operations.
 */
export function useSubscriptions(): UseSubscriptionsReturn {
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const subs = await api.getSubscriptions();
      setSubscriptions(subs);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const addSubscription = useCallback(
    async (url: string): Promise<Subscription> => {
      const sub = await api.addSubscription(url);
      setSubscriptions((prev) => [...prev, sub]);
      return sub;
    },
    [],
  );

  const deleteSubscription = useCallback(async (id: string) => {
    await api.deleteSubscription(id);
    setSubscriptions((prev) => prev.filter((s) => s.id !== id));
  }, []);

  const togglePause = useCallback(async (id: string) => {
    const updated = await api.toggleSubscriptionPause(id);
    setSubscriptions((prev) =>
      prev.map((s) => (s.id === id ? updated : s)),
    );
  }, []);

  const updateQuality = useCallback(
    async (id: string, quality: string) => {
      const updated = await api.updateSubscriptionQuality(id, quality);
      setSubscriptions((prev) =>
        prev.map((s) => (s.id === id ? updated : s)),
      );
    },
    [],
  );

  const updateGroup = useCallback(async (id: string, groupName: string) => {
    const updated = await api.updateSubscriptionGroup(id, groupName);
    setSubscriptions((prev) => prev.map((s) => (s.id === id ? updated : s)));
  }, []);

  useEffect(() => {
    setLoading(true);
    refresh().finally(() => setLoading(false));
  }, [refresh]);

  return {
    subscriptions,
    loading,
    error,
    refresh,
    addSubscription,
    deleteSubscription,
    togglePause,
    updateQuality,
    updateGroup,
  };
}
