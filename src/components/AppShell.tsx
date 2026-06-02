import { useState, useEffect, useCallback } from "react";
import type { Subscription, DownloadRecord, DownloadProgress, DownloadTask } from "@/types";
import TopBar from "./TopBar";
import StatusBar from "./StatusBar";
import SubscriptionList from "./SubscriptionList";
import AddSubscriptionDialog from "./AddSubscriptionDialog";
import DetailPanel from "./DetailPanel";
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

  const selectedSub = subscriptions.find((s) => s.id === selectedId) ?? null;
  const filteredRecords = selectedId
    ? records.filter((r) => r.subscription_id === selectedId)
    : [];

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
    const interval = setInterval(refreshQueue, 3000);
    return () => clearInterval(interval);
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
        {/* Sidebar — Subscription List */}
        <div
          className={`flex-shrink-0 border-r border-gray-200 dark:border-gray-700 transition-all duration-200 ${
            sidebarCollapsed ? "w-0 overflow-hidden border-none" : "w-[280px]"
          }`}
        >
          <SubscriptionList
            subscriptions={subscriptions}
            loading={subscriptionsLoading}
            error={subscriptionsError}
            selectedId={selectedId}
            onSelect={onSelectSubscription}
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

        {/* Main Panel */}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="flex-1 overflow-y-auto">
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
          </div>
          <StatusBar />
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
