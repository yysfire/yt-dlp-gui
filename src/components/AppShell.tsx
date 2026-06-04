import { useState, useEffect, useCallback } from "react";
import {
  FolderOpen as FolderOpenIcon,
} from "@mui/icons-material";
import type { Subscription, DownloadRecord, DownloadProgress, DownloadTask, QueueState } from "@/types";
import { listen } from "@tauri-apps/api/event";
import TopBar from "./TopBar";
import StatusBar from "./StatusBar";
import SubscriptionList from "./SubscriptionList";
import AddSubscriptionDialog from "./AddSubscriptionDialog";
import DetailPanel from "./DetailPanel";
import DownloadedList from "./DownloadedList";
import SettingsDialog from "./SettingsDialog";
import ExportDialog from "./ExportDialog";
import ImportDialog from "./ImportDialog";
import * as api from "@/lib/tauri";

interface AppShellProps {
  subscriptions: Subscription[];
  subscriptionsLoading: boolean;
  subscriptionsError: string | null;
  recordsError: string | null;
  selectedId: string | null;
  onSelectSubscription: (id: string | null) => void;
  onAddSubscription: (url: string) => Promise<Subscription>;
  onDeleteSubscription: (id: string) => Promise<void>;
  onTogglePause: (id: string) => Promise<void>;
  onRefreshSubscriptions: () => Promise<void>;
  records: DownloadRecord[];
  onCheckSubscription: (id: string) => Promise<void>;
  onManualCheckAll: () => Promise<void>;
  onUpdateGroup: (id: string, groupName: string) => Promise<void>;
  progressMap?: Map<string, DownloadProgress>;
}

/**
 * Main application shell: two-column layout with top bar and status bar.
 */
export default function AppShell({
  subscriptions,
  subscriptionsLoading,
  subscriptionsError,
  recordsError,
  selectedId,
  onSelectSubscription,
  onAddSubscription,
  onDeleteSubscription,
  onTogglePause,
  onRefreshSubscriptions,
  records,
  onCheckSubscription,
  onManualCheckAll,
  onUpdateGroup,
  progressMap,
}: AppShellProps) {
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [exportDialogOpen, setExportDialogOpen] = useState(false);
  const [importDialogOpen, setImportDialogOpen] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [queueTasks, setQueueTasks] = useState<DownloadTask[]>([]);
  const [activeView, setActiveView] = useState<"detail" | "downloads">("detail");

  const selectedSub = subscriptions.find((s) => s.id === selectedId) ?? null;
  const filteredRecords = selectedId
    ? records.filter((r) => r.subscription_id === selectedId)
    : [];
  const downloadCount = records.filter((r) => r.status !== "deleted").length;

  const refreshQueue = useCallback(async () => {
    try {
      const tasks = await api.getDownloadQueue();
      setQueueTasks(tasks);
    } catch {
      // Ignore errors
    }
  }, []);

  useEffect(() => {
    refreshQueue();
    const unlistenQueuePromise = listen<QueueState>("queue-changed", () => {
      refreshQueue();
    });
    const unlistenRecordsPromise = listen("records-changed", () => {
      refreshQueue();
    });
    return () => {
      unlistenQueuePromise.then((fn) => fn());
      unlistenRecordsPromise.then((fn) => fn());
    };
  }, [refreshQueue]);

  const handlePause = useCallback(async (videoUrl: string) => {
    try {
      await api.pauseDownloadByUrl(videoUrl);
      refreshQueue();
    } catch (e) {
      console.error("Failed to pause:", e);
    }
  }, [refreshQueue]);

  const handleCancel = useCallback(async (videoUrl: string) => {
    try {
      await api.cancelDownloadByUrl(videoUrl);
      refreshQueue();
    } catch (e) {
      console.error("Failed to cancel:", e);
    }
  }, [refreshQueue]);

  const handleResume = useCallback(async (taskId: string) => {
    try {
      await api.resumeDownload(taskId);
      refreshQueue();
    } catch (e) {
      console.error("Failed to resume:", e);
    }
  }, [refreshQueue]);

  const handleRetry = useCallback(async (subscriptionId: string) => {
    try {
      await api.checkSubscription(subscriptionId);
    } catch (e) {
      console.error("Failed to retry:", e);
    }
  }, []);

  return (
    <div className="flex flex-col h-screen bg-gray-50 dark:bg-gray-900 text-gray-900 dark:text-gray-100">
      {/* Top Bar */}
      <TopBar
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenAddDialog={() => setAddDialogOpen(true)}
        onToggleSidebar={() => setSidebarCollapsed((v) => !v)}
      />

      {/* Main Content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <div
          className={`flex-shrink-0 flex flex-col border-r border-gray-200 dark:border-gray-700 transition-all duration-200 ${
            sidebarCollapsed ? "w-0 overflow-hidden border-none" : "w-[280px]"
          }`}
        >
          {/* Subscription list — always visible */}
          <div className="flex-1 min-h-0">
            <SubscriptionList
              subscriptions={subscriptions}
              loading={subscriptionsLoading}
              error={subscriptionsError}
              selectedId={selectedId}
              onSelect={(id) => {
                setActiveView("detail");
                onSelectSubscription(id);
              }}
              onDelete={onDeleteSubscription}
              onTogglePause={onTogglePause}
              onCheckSubscription={onCheckSubscription}
              onManualCheckAll={onManualCheckAll}
              onRefresh={onRefreshSubscriptions}
              onOpenExport={() => setExportDialogOpen(true)}
              onOpenImport={() => setImportDialogOpen(true)}
              onUpdateGroup={onUpdateGroup}
            />
          </div>

          {/* Bottom: "已下载" navigation button */}
          {!sidebarCollapsed && (
            <button
              onClick={() => {
                setActiveView("downloads");
                onSelectSubscription(null);
              }}
              className={`flex items-center gap-2 w-full px-3 py-2.5 text-sm font-medium border-t border-gray-200 dark:border-gray-700 transition-colors
                ${activeView === "downloads"
                  ? "text-primary-600 dark:text-primary-400 bg-primary-50 dark:bg-primary-900/20"
                  : "text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800"}`}
            >
              <FolderOpenIcon fontSize="small" />
              <span className="flex-1 text-left">已下载</span>
              <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">
                {downloadCount}
              </span>
            </button>
          )}
        </div>

        {/* Main Panel */}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="flex-1 overflow-y-auto">
            {activeView === "downloads" ? (
              <DownloadedList
                records={records}
                onRefresh={onRefreshSubscriptions}
              />
            ) : (
              <DetailPanel
                subscription={selectedSub}
                records={filteredRecords}
                queueTasks={queueTasks}
                error={recordsError}
                progressMap={progressMap}
                onPauseDownload={handlePause}
                onResumeDownload={handleResume}
                onCancelDownload={handleCancel}
                onRetryDownload={handleRetry}
              />
            )}
          </div>
          <StatusBar refreshTrigger={activeView === "detail" ? selectedId : undefined} />
        </div>
      </div>

      {/* Dialogs */}
      <AddSubscriptionDialog
        open={addDialogOpen}
        onClose={() => setAddDialogOpen(false)}
        onAdd={onAddSubscription}
      />
      <SettingsDialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />
      <ExportDialog
        open={exportDialogOpen}
        onClose={() => setExportDialogOpen(false)}
        subscriptions={subscriptions}
      />
      <ImportDialog
        open={importDialogOpen}
        onClose={() => setImportDialogOpen(false)}
        onImported={onRefreshSubscriptions}
      />
    </div>
  );
}
