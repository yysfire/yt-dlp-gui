import { useState } from "react";
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  Button,
  CircularProgress,
  Alert,
} from "@mui/material";
import type { Subscription } from "@/types";

interface AddSubscriptionDialogProps {
  open: boolean;
  onClose: () => void;
  onAdd: (url: string) => Promise<Subscription>;
}

/** Dialog for adding a new channel subscription by URL. */
export default function AddSubscriptionDialog({
  open,
  onClose,
  onAdd,
}: AddSubscriptionDialogProps) {
  const [url, setUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    const trimmed = url.trim();
    if (!trimmed) return;

    setLoading(true);
    setError(null);
    try {
      await onAdd(trimmed);
      setUrl("");
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleClose = () => {
    if (!loading) {
      setUrl("");
      setError(null);
      onClose();
    }
  };

  return (
    <Dialog
      open={open}
      onClose={handleClose}
      maxWidth="sm"
      fullWidth
      disableRestoreFocus
    >
      <DialogTitle sx={{ fontWeight: 600, fontSize: "1.1rem" }}>
        添加订阅
      </DialogTitle >

      <DialogContent>
        {error && (
          <Alert severity="error" sx={{ mb: 2 }} variant="outlined">
            {error}
          </Alert>
        )}

        <TextField
          autoFocus
          fullWidth
          label="频道 URL"
          placeholder="https://www.youtube.com/@channel or https://space.bilibili.com/..."
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !loading) {
              handleSubmit();
            }
          }}
          disabled={loading}
          margin="dense"
          size="small"
          helperText="支持 YouTube 频道和 Bilibili 空间链接"
        />
      </DialogContent>

      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={handleClose} disabled={loading} size="small">
          取消
        </Button>
        <Button
          variant="contained"
          onClick={handleSubmit}
          disabled={loading || !url.trim()}
          size="small"
          startIcon={loading ? <CircularProgress size={14} /> : undefined}
        >
          {loading ? "解析中..." : "添加"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
