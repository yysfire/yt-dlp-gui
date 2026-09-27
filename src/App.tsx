import { useState, useEffect } from "react";
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

  // Listen for download-complete events from the Rust backend
  useEffect(() => {
    const unlistenPromise = listen<{ title: string; channel: string }>(
      "download-complete",
      (_event) => {
        refreshRecords();
      },
    );
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [refreshRecords]);

  // Listen for records-changed events (real-time status updates during check)
  useEffect(() => {
    const unlistenPromise = listen("records-changed", () => {
      refreshRecords();
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [refreshRecords]);

  // Listen for scheduler-check-complete (background check finished)
  useEffect(() => {
    const unlistenPromise = listen("scheduler-check-complete", () => {
      refreshRecords();
      refreshSubs();
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [refreshRecords, refreshSubs]);

  // Listen for subscriptions-updated (batch import / batch delete completed)
  useEffect(() => {
    const unlistenPromise = listen("subscriptions-updated", () => {
      refreshSubs();
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [refreshSubs]);

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
