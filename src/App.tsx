import { useState, useEffect, useCallback } from "react";
import {
  ThemeProvider,
  createTheme,
  CssBaseline,
} from "@mui/material";
import { listen } from "@tauri-apps/api/event";
import { useSubscriptions } from "@/hooks/useSubscriptions";
import { useDownloadRecords } from "@/hooks/useDownloadRecords";
import { useDownloadProgress } from "@/hooks/useDownloadProgress";
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
  const [selectedId, setSelectedId] = useState<string | null>(null);

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

  // Load dark mode preference from settings on mount
  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings: AppSettings = await api.getSettings();
        setDarkMode(settings.dark_mode);
        // Apply dark class to html element for Tailwind
        document.documentElement.classList.toggle(
          "dark",
          settings.dark_mode,
        );
      } catch {
        // Default to light mode
      }
    };
    loadSettings();
  }, []);

  // Start the scheduler on mount
  useEffect(() => {
    const initScheduler = async () => {
      try {
        await api.startScheduler();
      } catch {
        // Scheduler start is best-effort
      }
    };
    initScheduler();
  }, []);

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

  // Handle dark mode toggle (called when settings are updated externally)
  const handleDarkModeChange = useCallback((isDark: boolean) => {
    setDarkMode(isDark);
    document.documentElement.classList.toggle("dark", isDark);
  }, []);

  // After settings are saved, re-check dark mode
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const settings = await api.getSettings();
        if (settings.dark_mode !== darkMode) {
          handleDarkModeChange(settings.dark_mode);
        }
      } catch {
        // Ignore poll errors
      }
    }, 5000);
    return () => clearInterval(interval);
  }, [darkMode, handleDarkModeChange]);

  const theme = darkMode ? darkTheme : lightTheme;

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <AppShell
        subscriptions={subscriptions}
        subscriptionsLoading={subsLoading}
        subscriptionsError={subsError}
        selectedId={selectedId}
        onSelectSubscription={setSelectedId}
        onAddSubscription={addSubscription}
        onDeleteSubscription={deleteSubscription}
        onTogglePause={togglePause}
        onRefreshSubscriptions={refreshSubs}
        records={records}
        recordsError={recordsError}
        onCheckSubscription={async (id) => {
          await checkSubscription(id);
        }}
        onManualCheckAll={async () => {
          await checkAll();
        }}
        onUpdateGroup={updateGroup}
        progressMap={progressMap}
      />
    </ThemeProvider>
  );
}
