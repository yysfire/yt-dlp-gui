import { useState } from "react";
import {
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Avatar,
  Menu,
  MenuItem,
  ListItemSecondaryAction,
  Chip,
  Typography,
  IconButton,
  Select,
  FormControl,
} from "@mui/material";
import {
  MoreVert as MoreIcon,
  Pause as PauseIcon,
  PlayArrow as ResumeIcon,
  YouTube as YouTubeIcon,
  SmartDisplay as BilibiliIcon,
  Language as OtherIcon,
} from "@mui/icons-material";
import type { Subscription } from "@/types";
import { GROUPS } from "@/types";


interface SubscriptionItemProps {
  subscription: Subscription;
  selected: boolean;
  onSelect: () => void;
  onDelete: () => void;
  onTogglePause: () => void;
  onCheck: () => void;
  onUpdateGroup: (id: string, groupName: string) => void;
  keyword?: string;
}

/**
 * 字面搜索高亮 - 递归生成 JSX。
 * 在文本中查找 keyword，将匹配部分用 <mark> 标签包裹。
 * 使用正则特殊字符转义，仅做字面匹配，不触发正则语义。
 */
function highlightText(text: string, keyword: string | undefined): (string | React.JSX.Element)[] {
  if (!keyword || !text) {
    return [text];
  }

  const lowerText = text.toLowerCase();
  const lowerKw = keyword.toLowerCase();

  if (!lowerText.includes(lowerKw)) {
    return [text];
  }

  // 转义正则特殊字符，实现字面匹配
  const escapedKw = keyword.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const regex = new RegExp(`(${escapedKw})`, "gi");
  const parts = text.split(regex);

  const result: (string | React.JSX.Element)[] = [];
  for (let i = 0; i < parts.length; i++) {
    if (parts[i] === "") continue;
    if (parts[i].toLowerCase() === lowerKw) {
      result.push(
        <mark key={i} className="bg-yellow-200 dark:bg-yellow-600">
          {parts[i]}
        </mark>,
      );
    } else {
      result.push(parts[i]);
    }
  }

  return result.length > 0 ? result : [text];
}

/** Platform icon mapping. */
function PlatformIcon({ platform }: { platform: string }) {
  switch (platform) {
    case "youtube":
      return <YouTubeIcon fontSize="small" sx={{ color: "#FF0000" }} />;
    case "bilibili":
      return <BilibiliIcon fontSize="small" sx={{ color: "#00A1D6" }} />;
    default:
      return <OtherIcon fontSize="small" />;
  }
}

/** Single subscription row in the sidebar list. */
export default function SubscriptionItem({
  subscription,
  selected,
  onSelect,
  onDelete,
  onTogglePause,
  onCheck,
  onUpdateGroup,
  keyword,
}: SubscriptionItemProps) {
  const [anchorEl, setAnchorEl] = useState<HTMLElement | null>(null);
  const [editingGroup, setEditingGroup] = useState(false);
  const menuOpen = Boolean(anchorEl);

  const handleMenuOpen = (e: React.MouseEvent<HTMLElement>) => {
    e.stopPropagation();
    setAnchorEl(e.currentTarget);
  };

  const handleMenuClose = () => setAnchorEl(null);

  return (
    <>
      <ListItemButton
        selected={selected}
        onClick={onSelect}
        sx={{
          opacity: subscription.paused ? 0.55 : 1,
          pr: 6, // leave room for the More button
          "&.Mui-selected": {
            bgcolor: "primary.50",
          },
        }}
      >
        <ListItemIcon sx={{ minWidth: 40 }}>
          <Avatar
            src={subscription.channel_avatar_url}
            sx={{ width: 28, height: 28 }}
          >
            {subscription.channel_name.charAt(0)}
          </Avatar>
        </ListItemIcon>

        <ListItemText
          primary={
            <Typography variant="body2" noWrap>
              {highlightText(subscription.channel_name, keyword)}
            </Typography>
          }
          secondary={
            <span className="flex items-center gap-1">
              <PlatformIcon platform={subscription.platform} />
              <Typography variant="caption" color="text.disabled">
                {subscription.platform}
              </Typography>
            </span>
          }
          sx={{ my: 0, overflow: "hidden" }}
        />

        {subscription.paused && (
          <Chip
            label="已暂停"
            size="small"
            variant="outlined"
            sx={{ mr: 0.5, height: 20, fontSize: "0.65rem", flexShrink: 0 }}
          />
        )}

        {editingGroup ? (
          <FormControl size="small" sx={{ minWidth: 72, flexShrink: 0 }} onClick={(e) => e.stopPropagation()}>
            <Select
              value={subscription.group_name || "未分组"}
              autoFocus
              onClose={() => setEditingGroup(false)}
              onChange={(e) => {
                onUpdateGroup(subscription.id, e.target.value as string);
                setEditingGroup(false);
              }}
              sx={{ height: 22, fontSize: "0.65rem" }}
            >
              {GROUPS.map((g) => (
                <MenuItem key={g} value={g} dense sx={{ fontSize: "0.7rem" }}>
                  {g}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
        ) : (
          <Chip
            label={subscription.group_name || "未分组"}
            size="small"
            variant="outlined"
            sx={{ mr: 0.5, height: 20, fontSize: "0.65rem", cursor: "pointer", flexShrink: 0 }}
            onClick={(e) => {
              e.stopPropagation();
              setEditingGroup(true);
            }}
          />
        )}

        <ListItemSecondaryAction>
          <IconButton edge="end" size="small" onClick={handleMenuOpen}>
            <MoreIcon fontSize="small" />
          </IconButton>
        </ListItemSecondaryAction>
      </ListItemButton>

      {/* Context Menu */}
      <Menu
        anchorEl={anchorEl}
        open={menuOpen}
        onClose={handleMenuClose}
        anchorOrigin={{ vertical: "center", horizontal: "right" }}
        transformOrigin={{ vertical: "center", horizontal: "left" }}
      >
        <MenuItem
          onClick={() => {
            handleMenuClose();
            onTogglePause();
          }}
          dense
        >
          {subscription.paused ? (
            <ResumeIcon fontSize="small" sx={{ mr: 1 }} />
          ) : (
            <PauseIcon fontSize="small" sx={{ mr: 1 }} />
          )}
          {subscription.paused ? "恢复" : "暂停"}
        </MenuItem>
        <MenuItem
          onClick={() => {
            handleMenuClose();
            onCheck();
          }}
          dense
        >
          <PauseIcon fontSize="small" sx={{ mr: 1 }} />
          手动检查
        </MenuItem>
        <MenuItem
          onClick={() => {
            handleMenuClose();
            onDelete();
          }}
          dense
          sx={{ color: "error.main" }}
        >
          删除
        </MenuItem>
      </Menu>
    </>
  );
}
