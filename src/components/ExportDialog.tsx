import { useState } from "react";
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  Radio,
  RadioGroup,
  FormControlLabel,
  FormControl,
  FormLabel,
  Typography,
  Snackbar,
  Alert,
} from "@mui/material";
import { save } from "@tauri-apps/plugin-dialog";
import {
  exportSubscriptionsJson,
  exportSubscriptionsOpml,
} from "@/lib/tauri";
import type { Subscription } from "@/types";

interface ExportDialogProps {
  open: boolean;
  onClose: () => void;
  subscriptions: Subscription[];
}

type ExportFormat = "json" | "opml";

/**
 * Dialog for exporting subscriptions as JSON or OPML.
 *
 * - RadioGroup lets the user choose between JSON and OPML format.
 * - On confirm, opens a native save dialog via @tauri-apps/plugin-dialog,
 *   then invokes the corresponding Rust command.
 * - Shows a success/error snackbar on completion.
 */
export default function ExportDialog({
  open,
  onClose,
  subscriptions,
}: ExportDialogProps) {
  const [format, setFormat] = useState<ExportFormat>("json");
  const [exporting, setExporting] = useState(false);
  const [snackbar, setSnackbar] = useState<{
    open: boolean;
    severity: "success" | "error";
    message: string;
  }>({ open: false, severity: "success", message: "" });

  const defaultName = format === "json" ? "subscriptions.json" : "subscriptions.opml";
  const filterName = format === "json" ? "JSON Files" : "OPML Files";
  const filterExt = format === "json" ? "json" : "opml";

  const handleExport = async () => {
    try {
      const filePath = await save({
        defaultPath: defaultName,
        filters: [
          {
            name: filterName,
            extensions: [filterExt],
          },
        ],
      });

      if (!filePath) {
        // User cancelled the save dialog
        return;
      }

      setExporting(true);

      if (format === "json") {
        await exportSubscriptionsJson(filePath);
      } else {
        await exportSubscriptionsOpml(filePath);
      }

      setSnackbar({
        open: true,
        severity: "success",
        message: `已导出 ${subscriptions.length} 个订阅`,
      });
      onClose();
    } catch (err) {
      setSnackbar({
        open: true,
        severity: "error",
        message: `导出失败: ${String(err)}`,
      });
    } finally {
      setExporting(false);
    }
  };

  return (
    <>
      <Dialog open={open} onClose={onClose} maxWidth="xs" fullWidth>
        <DialogTitle>导出订阅</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            当前共有 {subscriptions.length} 个订阅
          </Typography>

          <FormControl component="fieldset">
            <FormLabel component="legend">导出格式</FormLabel>
            <RadioGroup
              value={format}
              onChange={(e) => setFormat(e.target.value as ExportFormat)}
            >
              <FormControlLabel
                value="json"
                control={<Radio />}
                label="JSON（完整数据，可重新导入）"
              />
              <FormControlLabel
                value="opml"
                control={<Radio />}
                label="OPML（RSS 阅读器通用格式）"
              />
            </RadioGroup>
          </FormControl>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose} disabled={exporting}>
            取消
          </Button>
          <Button
            variant="contained"
            onClick={handleExport}
            disabled={exporting}
          >
            {exporting ? "导出中..." : "导出"}
          </Button>
        </DialogActions>
      </Dialog>

      <Snackbar
        open={snackbar.open}
        autoHideDuration={4000}
        onClose={() => setSnackbar((s) => ({ ...s, open: false }))}
        anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
      >
        <Alert
          onClose={() => setSnackbar((s) => ({ ...s, open: false }))}
          severity={snackbar.severity}
          variant="filled"
        >
          {snackbar.message}
        </Alert>
      </Snackbar>
    </>
  );
}
