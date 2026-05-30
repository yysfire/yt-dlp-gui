import { useState } from "react";
import type { Subscription, DownloadRecord } from "@/types";
import TopBar from "./TopBar";
import StatusBar from "./StatusBar";
import SubscriptionList from "./SubscriptionList";
import AddSubscriptionDialog from "./AddSubscriptionDialog";
import DetailPanel from "./DetailPanel";
import SettingsDialog from "./SettingsDialog";
import ExportDialog from "./ExportDialog";
import ImportDialog from "./ImportDialog";

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
}: AppShellProps) {
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [exportDialogOpen, setExportDialogOpen] = useState(false);
  const [importDialogOpen, setImportDialogOpen] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  const selectedSub = subscriptions.find((s) => s.id === selectedId) ?? null;
  const filteredRecords = selectedId
    ? records.filter((r) => r.subscription_id === selectedId)
    : [];

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
              error={recordsError}
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
