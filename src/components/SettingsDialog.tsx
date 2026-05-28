import { useState, useEffect, useCallback } from "react";
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  Button,
  Select,
  MenuItem,
  FormControl,
  InputLabel,
  Switch,
  FormControlLabel,
  CircularProgress,
  Alert,
  Grid,
  InputAdornment,
} from "@mui/material";
import type { AppSettings } from "@/types";
import * as api from "@/lib/tauri";

interface SettingsDialogProps {
  open: boolean;
  onClose: () => void;
}

const QUALITY_OPTIONS = ["best", "2160p", "1440p", "1080p", "720p", "480p"];

/** Settings dialog for configuring download directory, quality, proxy, etc. */
export default function SettingsDialog({
  open,
  onClose,
}: SettingsDialogProps) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await api.getSettings();
      setSettings(s);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) {
      load();
      setSuccess(false);
    }
  }, [open, load]);

  const handleSave = async () => {
    if (!settings) return;
    setSaving(true);
    setError(null);
    try {
      const updated = await api.updateSettings(settings);
      setSettings(updated);
      setSuccess(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const updateField = <K extends keyof AppSettings>(
    key: K,
    value: AppSettings[K],
  ) => {
    setSettings((prev) => (prev ? { ...prev, [key]: value } : prev));
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth="sm"
      fullWidth
      disableRestoreFocus
    >
      <DialogTitle sx={{ fontWeight: 600, fontSize: "1.1rem" }}>
        设置
      </DialogTitle>

      <DialogContent>
        {loading && (
          <div className="flex justify-center py-4">
            <CircularProgress size={24} />
          </div>
        )}

        {error && (
          <Alert severity="error" sx={{ mb: 2 }} variant="outlined">
            {error}
          </Alert>
        )}

        {success && (
          <Alert severity="success" sx={{ mb: 2 }} variant="outlined">
            设置已保存
          </Alert>
        )}

        {settings && (
          <Grid container spacing={2} sx={{ mt: 0.5 }}>
            {/* Download Directory */}
            <Grid item xs={12}>
              <TextField
                fullWidth
                label="下载目录"
                value={settings.download_dir}
                onChange={(e) => updateField("download_dir", e.target.value)}
                size="small"
              />
            </Grid>

            {/* yt-dlp Path */}
            <Grid item xs={12}>
              <TextField
                fullWidth
                label="yt-dlp 路径"
                value={settings.yt_dlp_path}
                onChange={(e) => updateField("yt_dlp_path", e.target.value)}
                size="small"
                helperText="可执行文件路径或命令名"
              />
            </Grid>

            {/* Quality Preset */}
            <Grid item xs={6}>
              <FormControl fullWidth size="small">
                <InputLabel>默认画质</InputLabel>
                <Select
                  value={settings.quality_preset}
                  label="默认画质"
                  onChange={(e) =>
                    updateField("quality_preset", e.target.value)
                  }
                >
                  {QUALITY_OPTIONS.map((q) => (
                    <MenuItem key={q} value={q}>
                      {q}
                    </MenuItem>
                  ))}
                </Select>
              </FormControl>
            </Grid>

            {/* Check Interval */}
            <Grid item xs={6}>
              <TextField
                fullWidth
                label="检查间隔（分钟）"
                type="number"
                value={settings.check_interval_minutes}
                onChange={(e) =>
                  updateField(
                    "check_interval_minutes",
                    Math.max(1, parseInt(e.target.value) || 60),
                  )
                }
                size="small"
                inputProps={{ min: 1 }}
              />
            </Grid>

            {/* Proxy URL */}
            <Grid item xs={12}>
              <TextField
                fullWidth
                label="代理地址"
                value={settings.proxy_url}
                onChange={(e) => updateField("proxy_url", e.target.value)}
                size="small"
                placeholder="http://127.0.0.1:7890"
                helperText="留空则不使用代理"
              />
            </Grid>

            {/* Cookie File */}
            <Grid item xs={12}>
              <TextField
                fullWidth
                label="Cookie 文件"
                value={settings.cookie_file}
                onChange={(e) => updateField("cookie_file", e.target.value)}
                size="small"
                placeholder="留空则不使用 Cookie 认证"
                helperText="Netscape 格式的 Cookie 文件，用于 YouTube 认证"
                InputProps={{
                  endAdornment: (
                    <InputAdornment position="end">
                      <Button
                        size="small"
                        variant="outlined"
                        onClick={async () => {
                          try {
                            const { open } = await import(
                              "@tauri-apps/plugin-dialog"
                            );
                            const selected = await open({
                              filters: [
                                { name: "Cookie Files", extensions: ["txt"] },
                              ],
                              multiple: false,
                            });
                            if (selected && typeof selected === "string") {
                              updateField("cookie_file", selected);
                            }
                          } catch {
                            // User cancelled
                          }
                        }}
                      >
                        选择文件
                      </Button>
                    </InputAdornment>
                  ),
                }}
              />
            </Grid>

            {/* Toggles */}
            <Grid item xs={6}>
              <FormControlLabel
                control={
                  <Switch
                    checked={settings.dark_mode}
                    onChange={(e) =>
                      updateField("dark_mode", e.target.checked)
                    }
                    size="small"
                  />
                }
                label="暗色模式"
              />
            </Grid>
            <Grid item xs={6}>
              <FormControlLabel
                control={
                  <Switch
                    checked={settings.notifications_enabled}
                    onChange={(e) =>
                      updateField(
                        "notifications_enabled",
                        e.target.checked,
                      )
                    }
                    size="small"
                  />
                }
                label="通知"
              />
            </Grid>
          </Grid>
        )}
      </DialogContent>

      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onClose} disabled={saving} size="small">
          取消
        </Button>
        <Button
          variant="contained"
          onClick={handleSave}
          disabled={saving || !settings}
          size="small"
          startIcon={saving ? <CircularProgress size={14} /> : undefined}
        >
          {saving ? "保存中..." : "保存"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
