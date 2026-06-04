import { useState, useMemo, useEffect, useCallback } from "react";
import {
  List,
  Box,
  Typography,
  IconButton,
  Tooltip,
  Alert,
  CircularProgress,
} from "@mui/material";
import { Refresh as RefreshIcon } from "@mui/icons-material";
import type { DownloadRecord } from "@/types";
import FileSearchBar from "./FileSearchBar";
import DownloadedItem from "./DownloadedItem";
import * as api from "@/lib/tauri";

interface DownloadedListProps {
  /** All download records (unfiltered). */
  records: DownloadRecord[];
  loading?: boolean;
  error?: string | null;
  onRefresh?: () => void;
}

/**
 * List view showing all downloaded videos with search and status indicators.
 * This is the main file management view, entered via the sidebar "已下载" nav item.
 */
export default function DownloadedList({
  records,
  loading = false,
  error = null,
  onRefresh,
}: DownloadedListProps) {
  const [search, setSearch] = useState("");
  const [missingPaths, setMissingPaths] = useState<Set<string>>(new Set());
  const [syncing, setSyncing] = useState(false);

  // Client-side search filtering (case-insensitive)
  const filtered = useMemo(() => {
    if (!search.trim()) return records;
    const lower = search.toLowerCase();
    return records.filter((r) => r.video_title.toLowerCase().includes(lower));
  }, [records, search]);

  // Run file existence sync
  const handleSync = useCallback(async () => {
    setSyncing(true);
    try {
      const results = await api.syncFileStates();
      const missing = new Set(
        results.filter((r) => !r.exists).map((r) => r.file_path)
      );
      setMissingPaths(missing);
    } catch (e) {
      console.error("Sync failed:", e);
    } finally {
      setSyncing(false);
    }
  }, []);

  // Auto-sync on first load
  useEffect(() => {
    if (records.length > 0) {
      handleSync();
    }
  }, [records.length > 0]); // eslint-disable-line react-hooks/exhaustive-deps

  // Listen for file-sync-complete events from the backend
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen<Array<{ file_path: string; exists: boolean }>>(
        "file-sync-complete",
        (event) => {
          const missing = new Set(
            event.payload.filter((r) => !r.exists).map((r) => r.file_path)
          );
          setMissingPaths(missing);
        }
      ).then((fn) => {
        unlisten = fn;
      });
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const handleDelete = useCallback(() => {
    onRefresh?.();
  }, [onRefresh]);

  if (loading) {
    return (
      <Box className="flex items-center justify-center h-full">
        <CircularProgress size={32} />
      </Box>
    );
  }

  return (
    <Box className="flex flex-col h-full p-3">
      {/* Header */}
      <Box className="flex items-center justify-between mb-2">
        <Typography variant="subtitle2" fontWeight={600}>
          已下载 ({records.length})
        </Typography>
        <Tooltip title="刷新文件状态">
          <IconButton size="small" onClick={handleSync} disabled={syncing}>
            {syncing ? (
              <CircularProgress size={18} />
            ) : (
              <RefreshIcon fontSize="small" />
            )}
          </IconButton>
        </Tooltip>
      </Box>

      {/* Search */}
      <FileSearchBar value={search} onChange={setSearch} />

      {/* Error */}
      {error && (
        <Alert severity="error" sx={{ mb: 1 }}>
          {error}
        </Alert>
      )}

      {/* Empty state */}
      {!loading && records.length === 0 && (
        <Box className="flex items-center justify-center flex-1">
          <Typography variant="body2" color="text.disabled" className="text-center">
            暂无已下载的视频
          </Typography>
        </Box>
      )}

      {/* Empty search result */}
      {!loading && records.length > 0 && filtered.length === 0 && (
        <Box className="flex items-center justify-center flex-1">
          <Typography variant="body2" color="text.disabled" className="text-center">
            未找到匹配的视频
          </Typography>
        </Box>
      )}

      {/* Video list */}
      {filtered.length > 0 && (
        <List dense disablePadding className="flex-1 overflow-y-auto">
          {filtered.map((r) => (
            <DownloadedItem
              key={r.id}
              record={r}
              fileMissing={missingPaths.has(r.file_path)}
              onDeleted={handleDelete}
            />
          ))}
        </List>
      )}
    </Box>
  );
}
