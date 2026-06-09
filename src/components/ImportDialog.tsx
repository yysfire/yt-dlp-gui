import { useState, useMemo, useEffect, useCallback } from "react";
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  Tabs,
  Tab,
  TextField,
  Typography,
  Box,
  Snackbar,
  Alert,
  Chip,
  LinearProgress,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Checkbox,
  FormControlLabel,
  Tooltip,
} from "@mui/material";
import {
  CheckCircle as OkIcon,
  Error as ErrorIcon,
  ContentCopy as DuplicateIcon,
  Cancel as CancelIcon,
} from "@mui/icons-material";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import {
  batchImportPreview,
  batchImportExecute,
  cancelImport,
} from "@/lib/tauri";
import type {
  ImportPreview,
  ImportPreviewItem,
  ImportSource,
  ImportProgressEvent,
  ImportCompleteEvent,
  ImportErrorItem,
} from "@/types";

interface ImportDialogProps {
  open: boolean;
  onClose: () => void;
  onImported: () => void;
}

type ImportTab = "paste" | "file";
type ImportStage = "input" | "preview" | "importing" | "complete";

/**
 * Dialog for batch importing subscriptions.
 *
 * Enhanced flow:
 * 1. Select source (paste URLs or choose file)
 * 2. Preview parsed items before importing
 * 3. Execute import with real-time progress tracking
 * 4. Display result summary with success/failed/skipped stats
 *
 * Supports cancellation during import.
 */
export default function ImportDialog({
  open,
  onClose,
  onImported,
}: ImportDialogProps) {
  // ── Input stage state ──────────────────────────────────────────
  const [tab, setTab] = useState<ImportTab>("paste");
  const [urlText, setUrlText] = useState("");
  const [filePath, setFilePath] = useState<string | null>(null);

  // ── Import flow state ──────────────────────────────────────────
  const [stage, setStage] = useState<ImportStage>("input");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [importProgress, setImportProgress] = useState<ImportProgressEvent | null>(null);
  const [importComplete, setImportComplete] = useState<ImportCompleteEvent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // ── Compute URL list from paste input ───────────────────────────
  const pasteUrls = useMemo(() => {
    return urlText
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.length > 0);
  }, [urlText]);

  // ── Reset all state when dialog opens/closes ───────────────────
  const resetState = useCallback(() => {
    setUrlText("");
    setFilePath(null);
    setStage("input");
    setPreview(null);
    setSkipDuplicates(true);
    setTaskId(null);
    setImportProgress(null);
    setImportComplete(null);
    setLoading(false);
    setError(null);
    setTab("paste");
  }, []);

  useEffect(() => {
    if (open) {
      resetState();
    }
  }, [open, resetState]);

  // ── Listen for import-progress events ──────────────────────────
  useEffect(() => {
    if (!open) return;

    const unlistenPromise = listen<ImportProgressEvent>(
      "import-progress",
      (event) => {
        setImportProgress(event.payload);
      },
    );

    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [open]);

  // ── Listen for import-complete events ──────────────────────────
  useEffect(() => {
    if (!open) return;

    const unlistenPromise = listen<ImportCompleteEvent>(
      "import-complete",
      (event) => {
        setImportComplete(event.payload);
        setStage("complete");
        onImported();
      },
    );

    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [open, onImported]);

  // ── Build import source from current input ────────────────────
  const buildSource = useCallback((): ImportSource => {
    if (tab === "paste") {
      return { type: "url_list", urls: pasteUrls };
    }
    if (filePath) {
      const ext = filePath.split(".").pop()?.toLowerCase();
      if (ext === "opml") {
        return { type: "opml", path: filePath };
      }
      return { type: "txt", path: filePath };
    }
    // Fallback: treat as URL list
    return { type: "url_list", urls: [] };
  }, [tab, pasteUrls, filePath]);

  // ── Handlers ────────────────────────────────────────────────────

  const handleSelectFile = async () => {
    try {
      const selected = await openFile({
        multiple: false,
        filters: [
          {
            name: "OPML / TXT",
            extensions: ["opml", "txt"],
          },
        ],
      });

      if (selected) {
        setFilePath(typeof selected === "string" ? selected : selected[0] ?? null);
      }
    } catch {
      // User cancelled
    }
  };

  /** Fetch preview from backend without creating subscriptions. */
  const handlePreview = async () => {
    setLoading(true);
    setError(null);

    try {
      const source = buildSource();
      const result = await batchImportPreview(source);
      setPreview(result);
      setStage("preview");
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  /** Execute the import as a background task. */
  const handleExecute = async () => {
    setLoading(true);
    setError(null);

    try {
      const source = buildSource();
      const id = await batchImportExecute(source, skipDuplicates);
      setTaskId(id);
      setStage("importing");
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  /** Cancel the running import task. */
  const handleCancelImport = async () => {
    if (!taskId) return;
    try {
      await cancelImport(taskId);
    } catch {
      // Best effort
    }
  };

  /** Go back from preview to input stage. */
  const handleBackToInput = () => {
    setStage("input");
    setPreview(null);
    setImportProgress(null);
    setImportComplete(null);
    setTaskId(null);
  };

  const handleClose = () => {
    resetState();
    onClose();
  };

  // ── Computed flags ──────────────────────────────────────────────

  const canPreview =
    !loading &&
    (tab === "paste" ? pasteUrls.length > 0 : filePath !== null);

  const canExecute = preview !== null && !loading && preview.items.length > 0;

  // ── Preview item counts ─────────────────────────────────────────
  const validCount = preview
    ? preview.items.filter((i) => !i.error).length
    : 0;
  const errorCount = preview
    ? preview.items.filter((i) => i.error !== null).length
    : 0;

  // ── Build preview table data ────────────────────────────────────
  const previewItems: ImportPreviewItem[] = preview?.items ?? [];

  // ── Progress percentage ─────────────────────────────────────────
  const progressPercent = importProgress
    ? Math.round((importProgress.completed / importProgress.total) * 100)
    : 0;

  return (
    <>
      <Dialog
        open={open}
        onClose={stage === "importing" ? undefined : handleClose}
        maxWidth={stage === "input" ? "sm" : "md"}
        fullWidth
      >
        <DialogTitle>
          {stage === "input" && "批量导入订阅"}
          {stage === "preview" && "预览导入"}
          {stage === "importing" && "正在导入..."}
          {stage === "complete" && "导入完成"}
        </DialogTitle>
        <DialogContent>
          {/* ── Stage: Input ──────────────────────────────────────── */}
          {stage === "input" && (
            <>
              {/* Mode tabs */}
              <Tabs
                value={tab}
                onChange={(_, v) => setTab(v as ImportTab)}
                sx={{ mb: 2 }}
              >
                <Tab label="粘贴 URL" value="paste" />
                <Tab label="从文件" value="file" />
              </Tabs>

              {/* Paste mode */}
              {tab === "paste" && (
                <Box>
                  <TextField
                    label="每行一个频道 URL"
                    multiline
                    rows={6}
                    fullWidth
                    variant="outlined"
                    value={urlText}
                    onChange={(e) => setUrlText(e.target.value)}
                    placeholder={
                      "https://youtube.com/@channel1\nhttps://youtube.com/@channel2"
                    }
                    disabled={loading}
                  />
                  <Typography
                    variant="caption"
                    color="text.secondary"
                    sx={{ mt: 0.5, display: "block" }}
                  >
                    已识别 <strong>{pasteUrls.length}</strong> 个 URL
                    {pasteUrls.length > 0 && (
                      <Chip
                        size="small"
                        label={`${new Set(pasteUrls).size} 个去重后`}
                        sx={{ ml: 1 }}
                        variant="outlined"
                      />
                    )}
                  </Typography>
                </Box>
              )}

              {/* File mode */}
              {tab === "file" && (
                <Box>
                  <Button
                    variant="outlined"
                    onClick={handleSelectFile}
                    disabled={loading}
                    fullWidth
                  >
                    选择文件
                  </Button>
                  {filePath && (
                    <Typography
                      variant="body2"
                      sx={{ mt: 1 }}
                      color="text.secondary"
                    >
                      已选择: {filePath}
                    </Typography>
                  )}
                  <Typography
                    variant="caption"
                    color="text.secondary"
                    sx={{ mt: 0.5, display: "block" }}
                  >
                    支持 .opml（RSS 阅读器导出）和 .txt（每行一个 URL）格式
                  </Typography>
                </Box>
              )}

              {/* Error display */}
              {error && (
                <Alert severity="error" sx={{ mt: 2 }} variant="outlined">
                  {error}
                </Alert>
              )}
            </>
          )}

          {/* ── Stage: Preview ────────────────────────────────────── */}
          {stage === "preview" && preview && (
            <Box>
              {/* Summary chips */}
              <Box sx={{ display: "flex", gap: 1, mb: 2, flexWrap: "wrap" }}>
                <Chip
                  icon={<OkIcon />}
                  label={`有效 ${validCount}`}
                  color="success"
                  size="small"
                  variant="outlined"
                />
                {preview.duplicates > 0 && (
                  <Chip
                    icon={<DuplicateIcon />}
                    label={`重复 ${preview.duplicates}`}
                    color="warning"
                    size="small"
                    variant="outlined"
                  />
                )}
                {errorCount > 0 && (
                  <Chip
                    icon={<ErrorIcon />}
                    label={`无效 ${errorCount}`}
                    color="error"
                    size="small"
                    variant="outlined"
                  />
                )}
                <Chip
                  label={`共 ${preview.total} 项`}
                  size="small"
                  variant="outlined"
                />
              </Box>

              {/* Skip duplicates toggle */}
              <FormControlLabel
                control={
                  <Checkbox
                    checked={skipDuplicates}
                    onChange={(e) => setSkipDuplicates(e.target.checked)}
                    size="small"
                  />
                }
                label="跳过重复的订阅"
                sx={{ mb: 1 }}
              />

              {/* Preview table */}
              {previewItems.length > 0 ? (
                <TableContainer
                  component={Paper}
                  variant="outlined"
                  sx={{ maxHeight: 300 }}
                >
                  <Table size="small" stickyHeader>
                    <TableHead>
                      <TableRow>
                        <TableCell sx={{ fontWeight: 600 }}>状态</TableCell>
                        <TableCell sx={{ fontWeight: 600 }}>标题/URL</TableCell>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {previewItems.map((item, idx) => (
                        <TableRow key={idx}>
                          <TableCell sx={{ whiteSpace: "nowrap" }}>
                            {item.error ? (
                              <Tooltip title={item.error}>
                                <ErrorIcon color="error" fontSize="small" />
                              </Tooltip>
                            ) : item.is_duplicate ? (
                              <Tooltip title="与已有订阅重复">
                                <DuplicateIcon color="warning" fontSize="small" />
                              </Tooltip>
                            ) : (
                              <Tooltip title="可导入">
                                <OkIcon color="success" fontSize="small" />
                              </Tooltip>
                            )}
                          </TableCell>
                          <TableCell>
                            {item.title && (
                              <Typography variant="body2" fontWeight={500}>
                                {item.title}
                              </Typography>
                            )}
                            <Typography
                              variant="caption"
                              color={item.error ? "error" : "text.secondary"}
                              sx={{
                                wordBreak: "break-all",
                                display: "block",
                              }}
                            >
                              {item.url}
                            </Typography>
                            {item.is_duplicate && (
                              <Chip
                                label="重复"
                                size="small"
                                color="warning"
                                variant="outlined"
                                sx={{ mt: 0.25 }}
                              />
                            )}
                            {item.error && (
                              <Chip
                                label={item.error}
                                size="small"
                                color="error"
                                variant="outlined"
                                sx={{ mt: 0.25 }}
                              />
                            )}
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </TableContainer>
              ) : (
                <Typography color="text.secondary" sx={{ mt: 2 }}>
                  没有可导入的项
                </Typography>
              )}
            </Box>
          )}

          {/* ── Stage: Importing ───────────────────────────────────── */}
          {stage === "importing" && (
            <Box sx={{ mt: 1 }}>
              {/* Progress bar */}
              <Box sx={{ mb: 2 }}>
                <Box
                  sx={{
                    display: "flex",
                    justifyContent: "space-between",
                    mb: 0.5,
                  }}
                >
                  <Typography variant="body2" color="text.secondary">
                    导入进度
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    {importProgress
                      ? `${importProgress.completed} / ${importProgress.total}`
                      : "准备中..."}
                  </Typography>
                </Box>
                <LinearProgress
                  variant="determinate"
                  value={progressPercent}
                  sx={{ height: 8, borderRadius: 4 }}
                />
                <Typography
                  variant="caption"
                  color="text.secondary"
                  sx={{ display: "block", textAlign: "center", mt: 0.5 }}
                >
                  {progressPercent}%
                </Typography>
              </Box>

              {/* Current item info */}
              {importProgress && (
                <Box
                  sx={{
                    p: 1.5,
                    bgcolor: "action.hover",
                    borderRadius: 1,
                    mb: 2,
                  }}
                >
                  <Typography variant="caption" color="text.secondary">
                    正在处理
                  </Typography>
                  {importProgress.current_title && (
                    <Typography variant="body2" fontWeight={500}>
                      {importProgress.current_title}
                    </Typography>
                  )}
                  <Typography
                    variant="caption"
                    sx={{ wordBreak: "break-all", display: "block" }}
                  >
                    {importProgress.current_url}
                  </Typography>
                </Box>
              )}
            </Box>
          )}

          {/* ── Stage: Complete ────────────────────────────────────── */}
          {stage === "complete" && importComplete && (
            <Box sx={{ mt: 1 }}>
              {/* Result summary */}
              <Box sx={{ display: "flex", gap: 1, mb: 2, flexWrap: "wrap" }}>
                <Chip
                  icon={<OkIcon />}
                  label={`成功 ${importComplete.success}`}
                  color="success"
                />
                <Chip
                  icon={<ErrorIcon />}
                  label={`失败 ${importComplete.failed}`}
                  color="error"
                />
                <Chip
                  icon={<DuplicateIcon />}
                  label={`跳过 ${importComplete.skipped}`}
                  color="warning"
                />
                <Chip
                  label={`共 ${importComplete.total} 项`}
                  variant="outlined"
                />
              </Box>

              {/* Error details */}
              {importComplete.errors.length > 0 && (
                <Box sx={{ mb: 2 }}>
                  <Typography
                    variant="subtitle2"
                    color="error"
                    gutterBottom
                  >
                    错误详情
                  </Typography>
                  <TableContainer
                    component={Paper}
                    variant="outlined"
                    sx={{ maxHeight: 200 }}
                  >
                    <Table size="small" stickyHeader>
                      <TableHead>
                        <TableRow>
                          <TableCell sx={{ fontWeight: 600 }}>URL</TableCell>
                          <TableCell sx={{ fontWeight: 600 }}>原因</TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {importComplete.errors.map(
                          (err: ImportErrorItem, idx: number) => (
                            <TableRow key={idx}>
                              <TableCell
                                sx={{
                                  maxWidth: 300,
                                  wordBreak: "break-all",
                                  fontSize: "0.8rem",
                                }}
                              >
                                {err.url}
                              </TableCell>
                              <TableCell sx={{ fontSize: "0.8rem" }}>
                                {err.reason}
                              </TableCell>
                            </TableRow>
                          ),
                        )}
                      </TableBody>
                    </Table>
                  </TableContainer>
                </Box>
              )}
            </Box>
          )}
        </DialogContent>

        {/* ── Actions ──────────────────────────────────────────────── */}
        <DialogActions>
          {/* Importing stage: cancel button */}
          {stage === "importing" && (
            <Button
              onClick={handleCancelImport}
              color="error"
              startIcon={<CancelIcon />}
            >
              取消导入
            </Button>
          )}

          {/* Input stage */}
          {stage === "input" && (
            <>
              <Button onClick={handleClose} disabled={loading}>
                取消
              </Button>
              <Button
                variant="contained"
                onClick={handlePreview}
                disabled={!canPreview}
              >
                {loading ? "解析中..." : "预览"}
              </Button>
            </>
          )}

          {/* Preview stage */}
          {stage === "preview" && (
            <>
              <Button onClick={handleBackToInput} disabled={loading}>
                返回
              </Button>
              <Button
                variant="contained"
                onClick={handleExecute}
                disabled={!canExecute}
              >
                {loading ? "导入中..." : "确认导入"}
              </Button>
            </>
          )}

          {/* Complete stage */}
          {stage === "complete" && (
            <Button variant="contained" onClick={handleClose}>
              关闭
            </Button>
          )}
        </DialogActions>
      </Dialog>

      {/* Success snackbar */}
      <Snackbar
        open={stage === "complete"}
        autoHideDuration={4000}
        onClose={() => {}}
        anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
      >
        {importComplete ? (
          <Alert severity="success" variant="filled">
            导入完成：成功 {importComplete.success} 个，失败{" "}
            {importComplete.failed} 个，跳过 {importComplete.skipped} 个
          </Alert>
        ) : undefined}
      </Snackbar>
    </>
  );
}
