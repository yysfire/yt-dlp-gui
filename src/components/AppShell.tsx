import { useState, useEffect, useCallback } from "react";
import {
  FolderOpen as FolderOpenIcon,
} from "@mui/icons-material";
import type { Subscription, DownloadRecord, DownloadProgress, DownloadTask, QueueState, HealthCheckSummary } from "@/types";
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
import HealthCheckPanel from "./HealthCheckPanel";
import * as api from "@/lib/tauri";

/** 主面板视图：detail 携带选中的订阅 id（null 表示未选中）；downloads 无「选中」概念 */
type AppView =
  | { kind: "detail"; subscriptionId: string | null }
  | { kind: "downloads" };

interface AppShellProps {
  subscriptions: Subscription[];
  subscriptionsLoading: boolean;
  subscriptionsError: string | null;
  recordsError: string | null;
  onAddSubscription: (url: string) => Promise<Subscription>;
  onDeleteSubscription: (id: string) => Promise<void>;
  onTogglePause: (id: string) => Promise<void>;
  onRefreshSubscriptions: () => Promise<void>;
  records: DownloadRecord[];
  onCheckSubscription: (id: string) => Promise<void>;
  onManualCheckAll: () => Promise<void>;
  onUpdateGroup: (id: string, groupName: string) => Promise<void>;
  progressMap?: Map<string, DownloadProgress>;
  healthChecking: boolean;
  healthProgress: { completed: number; total: number } | null;
  healthSummary: HealthCheckSummary | null;
  onHealthCheckAll: () => Promise<void>;
  onHealthCheckSelected: (ids: string[]) => Promise<void>;
  onHealthClearResults: () => void;
}

/**
 * Main application shell: two-column layout with top bar and status bar.
 */
export default function AppShell({
  subscriptions,
  subscriptionsLoading,
  subscriptionsError,
  recordsError,
  onAddSubscription,
  onDeleteSubscription,
  onTogglePause,
  onRefreshSubscriptions,
  records,
  onCheckSubscription,
  onManualCheckAll,
  onUpdateGroup,
  progressMap,
  healthChecking,
  healthProgress,
  healthSummary,
  onHealthCheckAll,
  onHealthCheckSelected,
  onHealthClearResults,
}: AppShellProps) {
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [exportDialogOpen, setExportDialogOpen] = useState(false);
  const [importDialogOpen, setImportDialogOpen] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [queueTasks, setQueueTasks] = useState<DownloadTask[]>([]);
  const [healthCheckOpen, setHealthCheckOpen] = useState(false);
  // 主面板视图与选中订阅合并为一个可辨识联合：downloads 分支在类型上不存在 id，
  // 因此「已下载视图 + 有选中项」这种非法组合无法表示，也不再需要手动清空选中。
  const [view, setView] = useState<AppView>({ kind: "detail", subscriptionId: null });

  // 供侧边栏高亮、记录过滤、状态栏刷新使用；downloads 视图下恒为 null
  const selectedId = view.kind === "detail" ? view.subscriptionId : null;

  const selectedSub = subscriptions.find((s) => s.id === selectedId) ?? null;
  const filteredRecords = selectedId
    ? records.filter((r) => r.subscription_id === selectedId)
    : [];
  const recordCount = records.length;
  // 徽标显示下载记录总数（含 deleted —— 删除文件时记录有意保留以便追溯）；
  // 状态栏只数其中仍处于 completed 的记录，两者口径不同、不要互相替换。
  const completedCount = records.filter((r) => r.status === "completed").length;

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
    // 只订 queue-changed：凡改变队列的操作都会 emit 它（records-changed 的语义是
    // 「持久化记录变化」，与内存队列无关）
    const unlistenPromise = listen<QueueState>("queue-changed", () => {
      refreshQueue();
    });
    return () => {
      unlistenPromise.then((fn) => fn());
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
              onSelect={(id) => setView({ kind: "detail", subscriptionId: id })}
              onDelete={onDeleteSubscription}
              onTogglePause={onTogglePause}
              onCheckSubscription={onCheckSubscription}
              onManualCheckAll={onManualCheckAll}
              onRefresh={onRefreshSubscriptions}
              onOpenExport={() => setExportDialogOpen(true)}
              onOpenImport={() => setImportDialogOpen(true)}
              onOpenHealthCheck={() => setHealthCheckOpen(true)}
              onUpdateGroup={onUpdateGroup}
            />
          </div>

          {/* Bottom: "已下载" navigation button */}
          {!sidebarCollapsed && (
            <button
              onClick={() => setView({ kind: "downloads" })}
              className={`flex items-center gap-2 w-full px-3 py-1 text-xs font-medium border-t border-gray-200 dark:border-gray-700 transition-colors
                ${view.kind === "downloads"
                  ? "text-primary-600 dark:text-primary-400 bg-primary-50 dark:bg-primary-900/20"
                  : "text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800"}`}
            >
              <FolderOpenIcon fontSize="small" />
              <span className="flex-1 text-left">已下载</span>
              <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">
                {recordCount}
              </span>
            </button>
          )}
        </div>

        {/* Main Panel */}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="flex-1 overflow-y-auto">
            {view.kind === "downloads" ? (
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
          <StatusBar
            completedCount={completedCount}
          />
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
      <HealthCheckPanel
        open={healthCheckOpen}
        onClose={() => setHealthCheckOpen(false)}
        subscriptions={subscriptions}
        isChecking={healthChecking}
        progress={healthProgress}
        summary={healthSummary}
        onCheckAll={onHealthCheckAll}
        onCheckSelected={onHealthCheckSelected}
        onClearResults={onHealthClearResults}
        onRefreshSubscriptions={onRefreshSubscriptions}
      />
    </div>
  );
}
