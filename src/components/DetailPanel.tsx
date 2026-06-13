import { useState, useEffect, useRef, useMemo } from "react";
import { Box, Typography, Avatar, Chip, Alert, Skeleton, List, ListItem, Link, IconButton } from "@mui/material";
import {
  CheckCircle as SuccessIcon,
  Error as ErrorIcon,
  HeartBroken as DeadIcon,
  PlayCircleOutline as VideoIcon,
  Pause as PauseIcon,
  Cancel as CancelIcon,
  Downloading as DownloadingIcon,
  Replay as ReplayIcon,
  HourglassEmpty as WaitingIcon,
} from "@mui/icons-material";
import type { Subscription, DownloadRecord, DownloadProgress, DownloadTask, ChannelInfo, VideoInfo, UnifiedVideoItem, VideoStatus } from "@/types";
import { getChannelInfo, getChannelVideos } from "@/lib/tauri";

function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString("zh-CN");
  } catch {
    return iso;
  }
}

/** 将秒数格式化为人类可读的时长字符串（如 "12:34", "0:30"） */
function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "";
  const totalSeconds = Math.round(seconds);
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }
  return `${m}:${String(s).padStart(2, "0")}`;
}

/** 格式化文件大小 */
function formatFileSize(bytes: number): string {
  if (bytes === 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  return `${size.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

/** 从 yt-dlp epoch 字段或 upload_date (YYYYMMDD) 转为 YYYY-MM-DD 字符串 */
function formatUploadDate(uploadDate: string | null | undefined, epoch: number | null | undefined): string {
  if (uploadDate && /^\d{8}$/.test(uploadDate)) {
    return `${uploadDate.slice(0, 4)}-${uploadDate.slice(4, 6)}-${uploadDate.slice(6, 8)}`;
  }
  if (epoch != null && epoch > 0) {
    const d = new Date(epoch * 1000);
    return d.toISOString().slice(0, 10);
  }
  return "";
}

/** 按状态排序优先级 (越小越靠前) */
function statusPriority(status: VideoStatus): number {
  switch (status) {
    case "downloading": return 0;
    case "waiting": return 1;
    case "paused": return 2;
    case "completed": return 3;
    case "new": return 4;
    case "cancelled": return 5;
    case "failed": return 6;
    default: return 9;
  }
}

/** 合并频道视频与下载记录为统一列表 */
function mergeToUnifiedItems(
  videoList: VideoInfo[],
  records: DownloadRecord[],
  queueTasks: DownloadTask[],
): UnifiedVideoItem[] {
  const map = new Map<string, UnifiedVideoItem>();

  // 辅助：生成 key（优先 video_id，回退 url）
  const makeKey = (id: string, url: string): string => {
    return id || url;
  };

  // 辅助：获取派生状态
  const deriveStatus = (item: UnifiedVideoItem): VideoStatus => {
    const qt = item.queueTask;
    const di = item.downloadInfo;
    // queue task 优先级最高
    if (qt) {
      if (qt.status === "running") return "downloading";
      if (qt.status === "paused") return "paused";
      if (qt.status === "waiting") return "waiting";
      if (qt.status === "cancelled") return "cancelled";
      if (qt.status === "failed") return "failed";
      if (qt.status === "completed") return "completed";
    }
    // 回退到 download record 状态
    if (di) {
      return di.status as VideoStatus;
    }
    return "new";
  };

  // 第一轮：遍历下载记录
  for (const r of records) {
    const key = makeKey(r.video_id, r.video_url);
    const existing = map.get(key);
    if (existing) {
      // 已存在：仅当没有 downloadInfo 时设置
      if (!existing.downloadInfo) {
        existing.downloadInfo = r;
        (existing as { status: VideoStatus }).status = deriveStatus(existing);
      }
    } else {
      const item: UnifiedVideoItem = {
        downloadInfo: r,
        channelInfo: undefined,
        queueTask: undefined,
        id: key,
        title: r.video_title,
        url: r.video_url,
        status: r.status as VideoStatus,
      };
      map.set(key, item);
    }
  }

  // 第二轮：遍历队列任务
  for (const t of queueTasks) {
    const key = makeKey(t.video_id, t.video_url);
    const existing = map.get(key);
    if (existing) {
      existing.queueTask = t;
      (existing as { status: VideoStatus }).status = deriveStatus(existing);
      // 用队列任务的最新标题更新
      if (t.video_title) {
        (existing as { title: string }).title = t.video_title;
      }
    } else {
      const item: UnifiedVideoItem = {
        downloadInfo: undefined,
        channelInfo: undefined,
        queueTask: t,
        id: key,
        title: t.video_title,
        url: t.video_url,
        status: t.status === "running" ? "downloading"
          : t.status === "paused" ? "paused"
          : t.status === "waiting" ? "waiting"
          : (t.status as VideoStatus),
      };
      map.set(key, item);
    }
  }

  // 第三轮：遍历频道视频
  for (const v of videoList) {
    const key = makeKey(v.id, v.url);
    const existing = map.get(key);
    if (existing) {
      existing.channelInfo = v;
      // 频道视频的标题通常更新
      (existing as { title: string }).title = v.title;
      // 如有频道视频 ID，用它
      if (v.id) {
        (existing as { id: string }).id = v.id;
      }
      // 重新派生状态
      (existing as { status: VideoStatus }).status = deriveStatus(existing);
    } else {
      const item: UnifiedVideoItem = {
        downloadInfo: undefined,
        channelInfo: v,
        queueTask: undefined,
        id: key,
        title: v.title,
        url: v.url,
        status: "new",
      };
      map.set(key, item);
    }
  }

  // 排序
  const items = Array.from(map.values());
  items.sort((a, b) => {
    const pa = statusPriority(a.status);
    const pb = statusPriority(b.status);
    if (pa !== pb) return pa - pb;
    // 同状态下按时间倒序
    const getTime = (item: UnifiedVideoItem): number => {
      if (item.downloadInfo?.downloaded_at) {
        return new Date(item.downloadInfo.downloaded_at).getTime();
      }
      if (item.queueTask?.created_at) {
        return new Date(item.queueTask.created_at).getTime();
      }
      // 频道视频按 upload_date
      if (item.channelInfo?.upload_date) {
        // YYYYMMDD → 转为可比较的时间
        return new Date(
          item.channelInfo.upload_date.slice(0, 4) + "-" +
          item.channelInfo.upload_date.slice(4, 6) + "-" +
          item.channelInfo.upload_date.slice(6, 8)
        ).getTime();
      }
      return 0;
    };
    return getTime(b) - getTime(a);
  });

  return items;
}

interface DetailPanelProps {
  subscription: Subscription | null;
  records: DownloadRecord[];
  queueTasks: DownloadTask[];
  error?: string | null;
  progressMap?: Map<string, DownloadProgress>;
  onPauseDownload: (videoUrl: string) => void;
  onResumeDownload: (taskId: string) => void;
  onCancelDownload: (videoUrl: string) => void;
  onRetryDownload: (subscriptionId: string) => void;
}

/** Right-side detail panel showing channel info and download records. */
export default function DetailPanel({
  subscription,
  records,
  queueTasks,
  error,
  progressMap,
  onPauseDownload,
  onCancelDownload,
  onRetryDownload,
}: DetailPanelProps) {
  // 异步加载状态
  const [channelInfo, setChannelInfo] = useState<ChannelInfo | null>(null);
  const [videoList, setVideoList] = useState<VideoInfo[]>([]);
  const [_videoPage, setVideoPage] = useState(1);
  const [_hasMore, setHasMore] = useState(false);
  const [_totalVideos, setTotalVideos] = useState(0);
  const [loadingChannel, setLoadingChannel] = useState(false);
  const [loadingVideos, setLoadingVideos] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  // 用于取消上一个请求
  const requestIdRef = useRef(0);

  // 当订阅变化时加载频道信息和视频列表
  useEffect(() => {
    if (!subscription) {
      setChannelInfo(null);
      setVideoList([]);
      setVideoPage(1);
      setHasMore(false);
      setTotalVideos(0);
      setLoadError(null);
      return;
    }

    // 如果频道已失效率，不发起请求
    if (subscription.health_status === "dead") {
      setChannelInfo(null);
      setVideoList([]);
      setLoadError("此频道已失效");
      setLoadingChannel(false);
      setLoadingVideos(false);
      return;
    }

    // 递增 requestId 以取消上一个正在进行的请求
    const currentRequestId = ++requestIdRef.current;

    const loadData = async () => {
      setLoadError(null);
      setLoadingChannel(true);
      setLoadingVideos(true);

      // 加载频道信息
      try {
        const info = await getChannelInfo(subscription.id);
        if (currentRequestId !== requestIdRef.current) return;
        setChannelInfo(info);
      } catch (e) {
        if (currentRequestId !== requestIdRef.current) return;
        console.warn("Failed to load channel info:", e);
      }
      setLoadingChannel(false);

      // 加载视频列表第一页
      let hasMoreVideos = false;
      try {
        const result = await getChannelVideos(subscription.id, 1, 10);
        if (currentRequestId !== requestIdRef.current) return;
        setVideoList(result.videos);
        setVideoPage(1);
        setHasMore(result.has_more);
        setTotalVideos(result.total);
        hasMoreVideos = result.has_more;
      } catch (e) {
        if (currentRequestId !== requestIdRef.current) return;
        setLoadError(String(e));
      }
      setLoadingVideos(false);

      // 后台自动加载剩余页面（闭包变量跟踪页码，无需额外 useEffect）
      if (currentRequestId === requestIdRef.current && hasMoreVideos) {
        let page = 2;
        while (currentRequestId === requestIdRef.current) {
          try {
            const res = await getChannelVideos(subscription.id, page, 10);
            if (currentRequestId !== requestIdRef.current) return;
            setVideoList((prev) => [...prev, ...res.videos]);
            setVideoPage(page);
            setHasMore(res.has_more);
            setTotalVideos(res.total);
            if (!res.has_more) break;
            page++;
          } catch {
            break;
          }
        }
      }
    };

    loadData();
  }, [subscription?.id]);

  const isDead = subscription?.health_status === "dead";

  if (!subscription) {
    return (
      <Box
        sx={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          height: "100%",
          color: "text.disabled",
        }}
      >
        <Typography variant="body2" color="text.secondary">
          选择左侧订阅查看详情
        </Typography>
      </Box>
    );
  }

  // 合并频道视频、下载记录、队列任务为统一列表
  const unifiedItems = useMemo(() => {
    return mergeToUnifiedItems(videoList, records, queueTasks);
  }, [videoList, records, queueTasks]);

  return (
    <div className="flex flex-col h-full">
      {/* Channel Header */}
      <Box
        sx={{
          p: 3,
          display: "flex",
          alignItems: "center",
          gap: 2,
          borderBottom: 1,
          borderColor: "divider",
        }}
      >
        {loadingChannel ? (
          <Skeleton variant="circular" width={56} height={56} />
        ) : (
          <Avatar
            src={channelInfo?.thumbnail_url || subscription.channel_avatar_url}
            sx={{ width: 56, height: 56 }}
          >
            {subscription.channel_name.charAt(0)}
          </Avatar>
        )}
        <div className="flex-1 min-w-0">
          {loadingChannel ? (
            <Skeleton variant="text" width={200} height={32} />
          ) : (
            <Typography variant="h6" fontWeight={600} noWrap>
              {channelInfo?.channel_name || subscription.channel_name}
            </Typography>
          )}
          <div className="flex items-center gap-2 mt-0.5">
            <Chip
              label={subscription.platform}
              size="small"
              variant="outlined"
              sx={{ height: 20, fontSize: "0.7rem" }}
            />
            <Chip
              label={subscription.quality_preset}
              size="small"
              variant="outlined"
              sx={{ height: 20, fontSize: "0.7rem" }}
            />
            {subscription.paused && (
              <Chip
                label="已暂停"
                size="small"
                color="warning"
                variant="outlined"
                sx={{ height: 20, fontSize: "0.7rem" }}
              />
            )}
            {isDead && (
              <Chip
                icon={<DeadIcon sx={{ fontSize: 14 }} />}
                label="已失效"
                size="small"
                color="error"
                sx={{ height: 20, fontSize: "0.7rem" }}
              />
            )}
          </div>
          {/* Per-subscription stats */}
          <div className="flex items-center gap-2 mt-1">
            <Typography variant="caption" color="text.secondary">
              已下载: {subscription.download_count} 个
            </Typography>
            {channelInfo?.subscriber_count && (
              <Typography variant="caption" color="text.secondary">
                · 订阅者: {channelInfo.subscriber_count}
              </Typography>
            )}
            {channelInfo?.video_count != null && (
              <Typography variant="caption" color="text.secondary">
                · 视频: {channelInfo.video_count}
              </Typography>
            )}
            {subscription.last_checked_at && (
              <Typography variant="caption" color="text.disabled">
                · 上次检查: {formatTime(subscription.last_checked_at)}
              </Typography>
            )}
            {subscription.last_check_status === "success" && (
              <SuccessIcon sx={{ fontSize: 14, color: "success.main" }} />
            )}
            {subscription.last_check_status === "failed" && (
              <ErrorIcon sx={{ fontSize: 14, color: "error.main" }} />
            )}
          </div>
          {channelInfo?.description && (
            <Typography
              variant="caption"
              color="text.secondary"
              component="div"
              className="mt-0.5 line-clamp-2"
            >
              {channelInfo.description}
            </Typography>
          )}
          {subscription.last_check_status === "failed" && subscription.last_check_error && (
            <Typography variant="caption" color="error.main" component="div" className="mt-0.5">
              错误: {subscription.last_check_error}
            </Typography>
          )}
          <Typography
            variant="caption"
            color="text.disabled"
            sx={{ mt: 0.5, display: "block" }}
            noWrap
          >
            {subscription.url}
          </Typography>
        </div>
      </Box>

      <div className="flex-1 overflow-y-auto">
        {/* 频道已失效率提示 */}
        {isDead && (
          <Alert severity="error" sx={{ m: 2 }} variant="outlined" icon={<DeadIcon />}>
            此频道已失效，无法获取视频列表。
            {subscription.last_health_check && (
              <Typography variant="caption" display="block" sx={{ mt: 0.5 }}>
                上次健康检查: {formatTime(subscription.last_health_check)}
              </Typography>
            )}
          </Alert>
        )}

        {/* 加载错误 */}
        {error && !isDead && (
          <Alert severity="error" sx={{ m: 2 }} variant="outlined">
            {error}
          </Alert>
        )}

        {loadError && !isDead && !error && (
          <Alert severity="warning" sx={{ m: 2 }} variant="outlined">
            {loadError}
          </Alert>
        )}

        {/* 视频列表区域（仅非失效率频道） */}
        {!isDead && (
          <>
            {loadingVideos && videoList.length === 0 ? (
              <div className="p-4 space-y-2">
                {Array.from({ length: 5 }).map((_, i) => (
                  <Skeleton key={i} variant="rounded" height={48} />
                ))}
              </div>
            ) : (
              <>
                {/* 统一视频列表 */}
                {unifiedItems.length > 0 && (
                  <Box sx={{ borderBottom: 1, borderColor: "divider" }}>
                    <Typography
                      variant="subtitle2"
                      color="text.secondary"
                      sx={{ px: 2, pt: 2, pb: 1 }}
                    >
                      全部视频 ({unifiedItems.length})
                    </Typography>
                    <List dense disablePadding>
                      {unifiedItems.map((item) => (
                        <ListItem
                          key={item.id}
                          sx={{
                            px: 2,
                            borderBottom: 1,
                            borderColor: "divider",
                          }}
                        >
                          {/* 第 1 列：状态图标（垂直居中） */}
                          <Box sx={{ width: 40, flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center" }}>
                            {item.status === "new" && <VideoIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                            {item.status === "downloading" && <DownloadingIcon sx={{ fontSize: 18, color: "info.main" }} />}
                            {item.status === "paused" && <PauseIcon sx={{ fontSize: 18, color: "warning.main" }} />}
                            {item.status === "waiting" && <WaitingIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                            {item.status === "completed" && <SuccessIcon sx={{ fontSize: 18, color: "success.main" }} />}
                            {item.status === "failed" && <ErrorIcon sx={{ fontSize: 18, color: "error.main" }} />}
                            {item.status === "cancelled" && <CancelIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                          </Box>

                          {/* 第 2 列：三行内容 */}
                          <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: 0.3 }}>
                            {/* 第 1 行：标题 */}
                            <Typography
                              variant="body2"
                              noWrap
                              component={Link}
                              href={item.url}
                              target="_blank"
                              rel="noopener noreferrer"
                              underline="hover"
                              color="inherit"
                              sx={{ fontSize: "0.8rem", lineHeight: 1.3 }}
                            >
                              {item.title}
                            </Typography>

                            {/* 第 2 行：左(状态+元信息) · 右(辅助信息) */}
                            <Box sx={{ display: "flex", alignItems: "center" }}>
                              <Typography variant="caption" sx={{ fontSize: "0.65rem", whiteSpace: "nowrap" }}>
                                {(() => {
                                  const date = formatUploadDate(item.channelInfo?.upload_date, item.channelInfo?.epoch);
                                  return (
                                    <>
                                      {item.status === "new" && (
                                        <>
                                          {date}
                                          {date && item.channelInfo?.duration != null && " · "}
                                          {item.channelInfo?.duration != null && formatDuration(item.channelInfo.duration)}
                                        </>
                                      )}
                                      {item.status === "waiting" && `等待中${date ? ` · ${date}` : ""}${item.channelInfo?.duration != null ? ` · ${formatDuration(item.channelInfo.duration)}` : ""}`}
                                      {item.status === "downloading" && `下载中${date ? ` · ${date}` : ""}${item.channelInfo?.duration != null ? ` · ${formatDuration(item.channelInfo.duration)}` : ""}`}
                                      {item.status === "paused" && `已暂停${date ? ` · ${date}` : ""}${item.channelInfo?.duration != null ? ` · ${formatDuration(item.channelInfo.duration)}` : ""}`}
                                      {item.status === "completed" && (
                                        <>
                                          <Box component="span" sx={{ color: "success.main", fontWeight: 500 }}>已完成</Box>
                                          {date && ` · ${date}`}
                                          {item.channelInfo?.duration != null && ` · ${formatDuration(item.channelInfo.duration)}`}
                                          {!item.channelInfo && item.downloadInfo && ` · ${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN")}`}
                                        </>
                                      )}
                                      {item.status === "failed" && (
                                        <>
                                          <Box component="span" sx={{ color: "error.main", fontWeight: 500 }}>失败</Box>
                                          {date && ` · ${date}`}
                                          {item.channelInfo?.duration != null && ` · ${formatDuration(item.channelInfo.duration)}`}
                                          {item.downloadInfo?.error_message && ` · ${item.downloadInfo.error_message}`}
                                          {!item.channelInfo && item.downloadInfo && ` · ${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN")}`}
                                        </>
                                      )}
                                      {item.status === "cancelled" && `已取消${date ? ` · ${date}` : ""}`}
                                    </>
                                  );
                                })()}
                              </Typography>
                              <Box sx={{ flex: 1 }} />
                              <Typography variant="caption" sx={{ fontSize: "0.65rem", color: "text.disabled", whiteSpace: "nowrap" }}>
                                {item.downloadInfo && item.status === "downloading" && `下载中`}
                                {item.downloadInfo && item.status === "completed" && item.downloadInfo.file_size > 0 && `${formatFileSize(item.downloadInfo.file_size)} · ${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN", { month: "short", day: "numeric" })}`}
                                {item.downloadInfo && item.status === "failed" && `${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN", { month: "short", day: "numeric" })}`}
                              </Typography>
                            </Box>

                            {/* 第 3 行：进度条（仅下载中） */}
                            {item.status === "downloading" && item.downloadInfo && (
                              <Box sx={{ height: 4, bgcolor: "grey.200", borderRadius: 2, mt: 0.5 }}>
                                <Box
                                  sx={{
                                    height: "100%",
                                    bgcolor: "info.main",
                                    borderRadius: 2,
                                    transition: "width 0.3s",
                                    width: `${Math.min(
                                      progressMap?.get(item.downloadInfo.video_url)?.percent ?? 0,
                                      100
                                    )}%`,
                                  }}
                                />
                              </Box>
                            )}
                          </Box>

                          {/* 第 3 列：操作按钮（垂直居中） */}
                          <Box
                            sx={{
                              width: 40,
                              flexShrink: 0,
                              ml: 1,
                              display: "flex",
                              flexDirection: "column",
                              alignItems: "center",
                              justifyContent: "center",
                              gap: 0.5,
                            }}
                          >
                            {item.queueTask && item.status === "downloading" && (
                              <IconButton size="small" onClick={() => onPauseDownload(item.url)} title="暂停">
                                <PauseIcon sx={{ fontSize: 18 }} />
                              </IconButton>
                            )}
                            {item.queueTask && (item.status === "downloading" || item.status === "waiting" || item.status === "paused") && (
                              <IconButton size="small" onClick={() => onCancelDownload(item.url)} title="取消" sx={{ color: "error.main" }}>
                                <CancelIcon sx={{ fontSize: 18 }} />
                              </IconButton>
                            )}
                            {item.status === "failed" && (
                              <IconButton size="small" onClick={() => onRetryDownload(subscription.id)} title="重试" sx={{ color: "warning.main" }}>
                                <ReplayIcon sx={{ fontSize: 18 }} />
                              </IconButton>
                            )}
                          </Box>
                        </ListItem>
                      ))}
                    </List>
                  </Box>
                )}

                {/* 空列表提示 */}
                {!loadingVideos && unifiedItems.length === 0 && !isDead && (
                  <Box sx={{ p: 3, textAlign: "center" }}>
                    <Typography variant="body2" color="text.secondary">
                      暂无视频
                    </Typography>
                  </Box>
                )}
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
