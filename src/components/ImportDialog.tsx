import { useState, useMemo } from "react";
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
} from "@mui/material";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { batchImportSubscriptions } from "@/lib/tauri";
import type { ImportResult } from "@/types";

interface ImportDialogProps {
  open: boolean;
  onClose: () => void;
  onImported: () => void;
}

type ImportTab = "paste" | "file";

/**
 * Dialog for batch importing subscriptions.
 *
 * Supports two modes via MUI Tabs:
 * - "Paste URL": multiline textarea where each line is a URL.
 * - "From File": opens a native file dialog for .txt or .opml files.
 *
 * After import, displays a result summary and triggers a list refresh.
 */
export default function ImportDialog({
  open,
  onClose,
  onImported,
}: ImportDialogProps) {
  const [tab, setTab] = useState<ImportTab>("paste");
  const [urlText, setUrlText] = useState("");
  const [filePath, setFilePath] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Compute URL list from paste input
  const pasteUrls = useMemo(() => {
    return urlText
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.length > 0);
  }, [urlText]);

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

  const handleImport = async () => {
    setImporting(true);
    setError(null);
    setResult(null);

    try {
      let urls: string[] = [];
      let fp: string | null = null;

      if (tab === "paste") {
        urls = pasteUrls;
      } else {
        fp = filePath;
      }

      const res = await batchImportSubscriptions(urls, fp ?? undefined);
      setResult(res);
      onImported();
    } catch (err) {
      setError(String(err));
    } finally {
      setImporting(false);
    }
  };

  const handleClose = () => {
    setUrlText("");
    setFilePath(null);
    setResult(null);
    setError(null);
    setTab("paste");
    onClose();
  };

  const canImport =
    !importing &&
    (tab === "paste" ? pasteUrls.length > 0 : filePath !== null);

  return (
    <>
      <Dialog open={open} onClose={handleClose} maxWidth="sm" fullWidth>
        <DialogTitle>批量导入订阅</DialogTitle>
        <DialogContent>
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
                placeholder={"https://youtube.com/@channel1\nhttps://youtube.com/@channel2"}
                disabled={importing}
              />
              <Typography variant="caption" color="text.secondary" sx={{ mt: 0.5, display: "block" }}>
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
                disabled={importing}
                fullWidth
              >
                选择文件
              </Button>
              {filePath && (
                <Typography variant="body2" sx={{ mt: 1 }} color="text.secondary">
                  已选择: {filePath}
                </Typography>
              )}
              <Typography variant="caption" color="text.secondary" sx={{ mt: 0.5, display: "block" }}>
                支持 .opml（RSS 阅读器导出）和 .txt（每行一个 URL）格式
              </Typography>
            </Box>
          )}

          {/* Progress */}
          {importing && <LinearProgress sx={{ mt: 2 }} />}

          {/* Result display */}
          {result && (
            <Box sx={{ mt: 2 }}>
              <Typography variant="subtitle2" gutterBottom>
                导入结果
              </Typography>
              <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap" }}>
                <Chip
                  label={`成功 ${result.success_count}`}
                  color="success"
                  size="small"
                />
                <Chip
                  label={`重复跳过 ${result.skipped_duplicates.length}`}
                  color="warning"
                  size="small"
                />
                <Chip
                  label={`无效跳过 ${result.skipped_invalid.length}`}
                  color="error"
                  size="small"
                />
              </Box>

              {result.imported.length > 0 && (
                <Box sx={{ mt: 1 }}>
                  <Typography variant="caption" color="text.secondary">
                    已导入:
                  </Typography>
                  {result.imported.map((sub) => (
                    <Chip
                      key={sub.id}
                      label={sub.channel_name}
                      size="small"
                      sx={{ m: 0.25 }}
                      variant="outlined"
                    />
                  ))}
                </Box>
              )}

              {result.skipped_duplicates.length > 0 && (
                <Typography variant="caption" color="warning.main" sx={{ display: "block", mt: 0.5 }}>
                  重复: {result.skipped_duplicates.join(", ")}
                </Typography>
              )}
            </Box>
          )}

          {/* Error display */}
          {error && (
            <Alert severity="error" sx={{ mt: 2 }} variant="outlined">
              {error}
            </Alert>
          )}
        </DialogContent>
        <DialogActions>
          <Button onClick={handleClose} disabled={importing}>
            {result ? "关闭" : "取消"}
          </Button>
          <Button
            variant="contained"
            onClick={handleImport}
            disabled={!canImport}
          >
            {importing ? "导入中..." : "导入"}
          </Button>
        </DialogActions>
      </Dialog>

      {/* Success snackbar — only when result is shown and dialog is still open */}
      <Snackbar
        open={result !== null}
        autoHideDuration={4000}
        onClose={() => setResult(null)}
        anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
      >
        {result ? (
          <Alert severity="success" variant="filled">
            导入完成：成功 {result.success_count} 个，跳过{" "}
            {result.skipped_duplicates.length + result.skipped_invalid.length} 个
          </Alert>
        ) : undefined}
      </Snackbar>
    </>
  );
}
