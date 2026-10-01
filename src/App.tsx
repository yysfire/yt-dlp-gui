import { useState, useEffect, useCallback, useRef } from "react";
import {
  ThemeProvider,
  createTheme,
  CssBaseline,
} from "@mui/material";
import { listen } from "@tauri-apps/api/event";
import { useSubscriptions } from "@/hooks/useSubscriptions";
import { useDownloadRecords } from "@/hooks/useDownloadRecords";
import { useDownloadProgress } from "@/hooks/useDownloadProgress";
import { useHealthCheck } from "@/hooks/useHealthCheck";
import AppShell from "@/components/AppShell";
import * as api from "@/lib/tauri";
import type { AppSettings } from "@/types";

/** Light theme palette. */
const lightTheme = createTheme({
  palette: {
    mode: "light",
    primary: { main: "#2563eb" },
    background: {
      default: "#f9fafb",
      paper: "#ffffff",
    },
  },
  typography: {
    fontFamily: '"Inter", "Roboto", "Helvetica", "Arial", sans-serif',
  },
  shape: { borderRadius: 8 },
  components: {
    MuiButton: {
      styleOverrides: {
        root: { textTransform: "none" },
      },
    },
    MuiDialog: {
      styleOverrides: {
        paper: { borderRadius: 12 },
      },
    },
  },
});

/** Dark theme palette. */
const darkTheme = createTheme({
  palette: {
    mode: "dark",
    primary: { main: "#60a5fa" },
    background: {
      default: "#111827",
      paper: "#1f2937",
    },
  },
  typography: {
    fontFamily: '"Inter", "Roboto", "Helvetica", "Arial", sans-serif',
  },
  shape: { borderRadius: 8 },
  components: {
    MuiButton: {
      styleOverrides: {
        root: { textTransform: "none" },
      },
    },
    MuiDialog: {
      styleOverrides: {
        paper: { borderRadius: 12 },
      },
    },
  },
});

export default function App() {
  const [darkMode, setDarkMode] = useState(false);

  const {
    subscriptions,
    loading: subsLoading,
    error: subsError,
    addSubscription,
    deleteSubscription,
    togglePause,
    updateGroup,
    refresh: refreshSubs,
  } = useSubscriptions();

  const {
    records,
    error: recordsError,
    checkSubscription,
    checkAll,
    refresh: refreshRecords,
  } = useDownloadRecords();

  const { progressMap } = useDownloadProgress();

  const {
    isChecking: healthChecking,
    progress: healthProgress,
    summary: healthSummary,
    startCheckAll: startHealthCheckAll,
    startCheckSelected: startHealthCheckSelected,
    clearResults: clearHealthResults,
  } = useHealthCheck(refreshSubs);

  // 唯一的事件合并点：同一批后端事件只触发一次 records + subs 全量拉取。
  //
  // 用「窗口内只排一次」而不是 debounce —— debounce 在连续入队（背靠背的同步循环）时
  // 会被不断推迟，永远不会刷新。150ms 足以把一批事件吞进同一窗口，又远低于人眼可感的
  // 阈值；交互反馈（暂停/取消/恢复）走 queue-changed 到 AppShell，不经过这里。
  const refreshTimerRef = useRef<number | null>(null);
  const scheduleRefresh = useCallback(() => {
    if (refreshTimerRef.current !== null) return; // 本窗口已排定 → 合并
    refreshTimerRef.current = window.setTimeout(() => {
      refreshTimerRef.current = null;
      void refreshRecords();
      void refreshSubs();
    }, 150);
  }, [refreshRecords, refreshSubs]);

  // StrictMode 双挂载下清理挂起的 timer，避免卸载后仍触发刷新
  useEffect(() => () => {
    if (refreshTimerRef.current !== null) {
      window.clearTimeout(refreshTimerRef.current);
      refreshTimerRef.current = null;
    }
  }, []);

  // Load dark mode preference from settings on mount
  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings: AppSettings = await api.getSettings();
        setDarkMode(settings.dark_mode);
      } catch {
        // Default to light mode
      }
    };
    loadSettings();
  }, []);

  // <html> 上的 dark class 是 darkMode 的唯一投影（Tailwind darkMode: "class" 依赖它）。
  // 不要在其他地方手写 classList.toggle。
  useEffect(() => {
    document.documentElement.classList.toggle("dark", darkMode);
  }, [darkMode]);

  // Listen for records-changed events (real-time status updates during check)
  useEffect(() => {
    const unlistenPromise = listen("records-changed", () => {
      scheduleRefresh();
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [scheduleRefresh]);

  // Listen for scheduler-check-complete (a check round finished — auto, tray, or manual)
  useEffect(() => {
    const unlistenPromise = listen("scheduler-check-complete", () => {
      scheduleRefresh();
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [scheduleRefresh]);

  // Listen for subscriptions-updated (batch import / batch delete completed)
  useEffect(() => {
    const unlistenPromise = listen("subscriptions-updated", () => {
      scheduleRefresh();
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [scheduleRefresh]);

  // Listen for settings-changed (settings were saved to disk and cache was synced)
  useEffect(() => {
    const unlistenPromise = listen<AppSettings>("settings-changed", (event) => {
      setDarkMode(event.payload.dark_mode);
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  const theme = darkMode ? darkTheme : lightTheme;

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <AppShell
        subscriptions={subscriptions}
        subscriptionsLoading={subsLoading}
        subscriptionsError={subsError}
        onAddSubscription={addSubscription}
        onDeleteSubscription={deleteSubscription}
        onTogglePause={togglePause}
        onRefreshSubscriptions={refreshSubs}
        records={records}
        recordsError={recordsError}
        onCheckSubscription={async (id) => {
          await checkSubscription(id);
          await refreshSubs();
          await refreshRecords();
        }}
        onManualCheckAll={async () => {
          await checkAll();
          await refreshSubs();
          await refreshRecords();
        }}
        onUpdateGroup={updateGroup}
        progressMap={progressMap}
        healthChecking={healthChecking}
        healthProgress={healthProgress}
        healthSummary={healthSummary}
        onHealthCheckAll={startHealthCheckAll}
        onHealthCheckSelected={startHealthCheckSelected}
        onHealthClearResults={clearHealthResults}
      />
    </ThemeProvider>
  );
}
