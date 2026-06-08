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
  Slider,
  Typography,
  Chip,
} from "@mui/material";
import FolderOpenIcon from "@mui/icons-material/FolderOpen";
import CheckCircleIcon from "@mui/icons-material/CheckCircle";
import ErrorIcon from "@mui/icons-material/Error";
import type { AppSettings, PathValidateResult, ProxyValidateResult } from "@/types";
import * as api from "@/lib/tauri";

interface SettingsDialogProps {
  open: boolean;
  onClose: () => void;
}

/** Quality options per spec: 最高画质, 1080p, 720p, 480p, 仅音频 */
const QUALITY_OPTIONS = [
  { value: "best", label: "最高画质" },
  { value: "1080p", label: "1080p" },
  { value: "720p", label: "720p" },
  { value: "480p", label: "480p" },
  { value: "audio", label: "仅音频" },
];

/** Check interval options: 手动, 30分钟, 每小时, 每天 */
const CHECK_INTERVAL_OPTIONS = [
  { value: 0, label: "手动" },
  { value: 30, label: "30 分钟" },
  { value: 60, label: "每小时" },
  { value: 1440, label: "每天" },
];

/** Settings dialog with download path, quality, proxy, concurrency, and scheduler frequency. */
export default function SettingsDialog({
  open,
  onClose,
}: SettingsDialogProps) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  // Validation states
  const [pathValidating, setPathValidating] = useState(false);
  const [pathResult, setPathResult] = useState<PathValidateResult | null>(null);
  const [proxyValidating, setProxyValidating] = useState(false);
  const [proxyResult, setProxyResult] = useState<ProxyValidateResult | null>(
    null,
  );

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await api.getSettings();
      setSettings(s);
      // Reset validation on reload
      setPathResult(null);
      setProxyResult(null);
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
      // Validate before save
      if (!canSave) {
        setError("请先修正设置中的错误项");
        return;
      }
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

  // ── Path validation ───────────────────────────────────────────

  const validatePath = async (path: string) => {
    if (!path.trim()) {
      setPathResult({
        valid: false,
        writable: false,
        exists: false,
        error: "下载路径不能为空",
      });
      return;
    }
    setPathValidating(true);
    setPathResult(null);
    try {
      const result = await api.validateDownloadPath(path);
      setPathResult(result);
    } catch (e) {
      setPathResult({
        valid: false,
        writable: false,
        exists: false,
        error: String(e),
      });
    } finally {
      setPathValidating(false);
    }
  };

  const handleBrowseFolder = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === "string") {
        updateField("download_dir", selected);
        validatePath(selected);
      }
    } catch {
      // User cancelled
    }
  };

  const handlePathBlur = () => {
    if (settings) {
      validatePath(settings.download_dir);
    }
  };

  // ── Proxy validation ──────────────────────────────────────────

  const validateProxy = async (url: string) => {
    if (!url.trim()) {
      setProxyResult({ valid: true, scheme: null, error: null });
      return;
    }
    setProxyValidating(true);
    setProxyResult(null);
    try {
      const result = await api.validateProxyUrl(url);
      setProxyResult(result);
    } catch (e) {
      setProxyResult({ valid: false, scheme: null, error: String(e) });
    } finally {
      setProxyValidating(false);
    }
  };

  const handleProxyBlur = () => {
    if (settings) {
      validateProxy(settings.proxy_url);
    }
  };

  // ── Save readiness ────────────────────────────────────────────

  const canSave =
    settings &&
    (pathResult === null || pathResult.valid) &&
    (proxyResult === null || proxyResult.valid);

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
            {/* ── Download Directory ─────────────────────────── */}
            <Grid item xs={12}>
              <TextField
                fullWidth
                label="下载目录"
                value={settings.download_dir}
                onChange={(e) => {
                  updateField("download_dir", e.target.value);
                  setPathResult(null); // reset validation on edit
                }}
                onBlur={handlePathBlur}
                size="small"
                error={pathResult !== null && !pathResult.valid}
                helperText={
                  pathResult?.error ?? (pathResult?.valid ? "路径有效" : "")
                }
                InputProps={{
                  endAdornment: (
                    <InputAdornment position="end">
                      {pathValidating ? (
                        <CircularProgress size={20} sx={{ mr: 0.5 }} />
                      ) : pathResult?.valid ? (
                        <CheckCircleIcon
                          color="success"
                          fontSize="small"
                          sx={{ mr: 0.5 }}
                        />
                      ) : pathResult && !pathResult.valid ? (
                        <ErrorIcon
                          color="error"
                          fontSize="small"
                          sx={{ mr: 0.5 }}
                        />
                      ) : null}
                      <Button
                        size="small"
                        variant="outlined"
                        startIcon={<FolderOpenIcon />}
                        onClick={handleBrowseFolder}
                      >
                        浏览
                      </Button>
                    </InputAdornment>
                  ),
                }}
              />
            </Grid>

            {/* ── yt-dlp Path ─────────────────────────────────── */}
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

            {/* ── Quality Preset ──────────────────────────────── */}
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
                    <MenuItem key={q.value} value={q.value}>
                      {q.label}
                    </MenuItem>
                  ))}
                </Select>
              </FormControl>
            </Grid>

            {/* ── Check Interval ──────────────────────────────── */}
            <Grid item xs={6}>
              <FormControl fullWidth size="small">
                <InputLabel>检查频率</InputLabel>
                <Select
                  value={settings.check_interval_minutes}
                  label="检查频率"
                  onChange={(e) =>
                    updateField(
                      "check_interval_minutes",
                      Number(e.target.value),
                    )
                  }
                >
                  {CHECK_INTERVAL_OPTIONS.map((opt) => (
                    <MenuItem key={opt.value} value={opt.value}>
                      {opt.label}
                    </MenuItem>
                  ))}
                </Select>
              </FormControl>
            </Grid>

            {/* ── Proxy URL ───────────────────────────────────── */}
            <Grid item xs={12}>
              <TextField
                fullWidth
                label="代理地址"
                value={settings.proxy_url}
                onChange={(e) => {
                  updateField("proxy_url", e.target.value);
                  setProxyResult(null); // reset on edit
                }}
                onBlur={handleProxyBlur}
                size="small"
                placeholder="http://127.0.0.1:7890"
                helperText={
                  proxyResult?.error ??
                  (proxyResult?.valid && proxyResult.scheme
                    ? `代理: ${proxyResult.scheme}://`
                    : "留空则不使用代理")
                }
                error={proxyResult !== null && !proxyResult.valid}
                InputProps={{
                  endAdornment: proxyValidating ? (
                    <InputAdornment position="end">
                      <CircularProgress size={20} />
                    </InputAdornment>
                  ) : proxyResult?.valid && proxyResult.scheme ? (
                    <InputAdornment position="end">
                      <CheckCircleIcon color="success" fontSize="small" />
                    </InputAdornment>
                  ) : proxyResult && !proxyResult.valid ? (
                    <InputAdornment position="end">
                      <ErrorIcon color="error" fontSize="small" />
                    </InputAdornment>
                  ) : null,
                }}
              />
            </Grid>

            {/* ── Cookie File ─────────────────────────────────── */}
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

            {/* ── Concurrent Downloads ────────────────────────── */}
            <Grid item xs={12}>
              <Typography variant="body2" gutterBottom>
                并发下载数
              </Typography>
              <div className="flex items-center gap-3">
                <Slider
                  value={settings.max_concurrent_downloads}
                  onChange={(_, val) =>
                    updateField(
                      "max_concurrent_downloads",
                      val as number,
                    )
                  }
                  min={1}
                  max={5}
                  step={1}
                  marks
                  valueLabelDisplay="auto"
                  sx={{ flex: 1 }}
                />
                <Chip
                  label={settings.max_concurrent_downloads}
                  color="primary"
                  size="small"
                  sx={{ minWidth: 36, fontWeight: 600 }}
                />
              </div>
              <Typography variant="caption" color="text.secondary">
                同时下载的最大任务数（1-5），调低时会按进度暂停进度最少的任务
              </Typography>
            </Grid>

            {/* ── Toggles ─────────────────────────────────────── */}
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

            {/* ── Tray Behavior ─────────────────────────────────── */}
            <Grid item xs={12}>
              <Typography
                variant="subtitle2"
                color="text.secondary"
                gutterBottom
                sx={{ mt: 1, fontWeight: 600 }}
              >
                系统托盘
              </Typography>
            </Grid>
            <Grid item xs={6}>
              <FormControlLabel
                control={
                  <Switch
                    checked={settings.minimize_to_tray}
                    onChange={(e) =>
                      updateField("minimize_to_tray", e.target.checked)
                    }
                    size="small"
                  />
                }
                label="最小化到托盘"
              />
            </Grid>
            <Grid item xs={6}>
              <FormControlLabel
                control={
                  <Switch
                    checked={settings.close_to_tray}
                    onChange={(e) =>
                      updateField("close_to_tray", e.target.checked)
                    }
                    size="small"
                  />
                }
                label="关闭到托盘"
              />
            </Grid>
            <Grid item xs={6}>
              <FormControlLabel
                control={
                  <Switch
                    checked={settings.start_in_tray}
                    onChange={(e) =>
                      updateField("start_in_tray", e.target.checked)
                    }
                    size="small"
                  />
                }
                label="启动时最小化到托盘"
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
          disabled={saving || !settings || !canSave}
          size="small"
          startIcon={saving ? <CircularProgress size={14} /> : undefined}
        >
          {saving ? "保存中..." : "保存"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
