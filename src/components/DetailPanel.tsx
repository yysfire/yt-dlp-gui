import { useState, useEffect, useRef, useMemo } from "react";
import { Box, Typography, Avatar, Chip, Alert, Skeleton, List, ListItem, Link, IconButton, Select, MenuItem } from "@mui/material";
import { alpha, type Theme } from "@mui/material/styles";
import {
  CheckCircle as SuccessIcon,
  Error as ErrorIcon,
  HeartBroken as DeadIcon,
  PlayCircleOutline as VideoIcon,
  Pause as PauseIcon,
  Cancel as CancelIcon,
  Downloading as DownloadingIcon,
  Replay as ReplayIcon,
  Autorenew as RetryingIcon,
  HourglassEmpty as WaitingIcon,
  DeleteOutline as DeletedIcon,
  SystemUpdateAlt as UpgradeIcon,
  FileDownload as RedownloadIcon,
} from "@mui/icons-material";
import type { Subscription, DownloadRecord, DownloadProgress, DownloadTask, ChannelInfo, VideoInfo, UnifiedVideoItem } from "@/types";
import { getChannelInfo, getChannelVideos } from "@/lib/tauri";
import {
  buildUnifiedVideoList,
  formatDuration,
  formatFileSize,
  formatTime,
  formatUploadDate,
} from "@/lib/unifiedVideoList";

/** 界面提供的画质档位（decisions §2.1：界面只给这五档）。 */
const QUALITY_OPTIONS = ["best", "1080p", "720p", "480p", "audio"] as const;

/** 画质档位的展示文案：`best` 渲染为「最高画质」。 */
function qualityLabel(quality: string): string {
  return quality === "best" ? "最高画质" : quality;
}

/** 重试次数上限（与后端 `retry_policy::MAX_ATTEMPTS` 对齐）。 */
const MAX_RETRY_ATTEMPTS = 3;

/**
 * 退避倒计时：每秒刷新一次剩余秒数。
 *
 * **只在详情面板使用** —— 列表视图每行挂一个计时器代价过高（decisions §8）。
 */
function RetryCountdown({ nextRetryAt }: { nextRetryAt: string }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  const target = new Date(nextRetryAt).getTime();
  const seconds = Number.isFinite(target)
    ? Math.max(0, Math.ceil((target - now) / 1000))
    : 0;
  return <>· 还有 {seconds} 秒</>;
}

/** 行内徽标的统一样式（高度/字号都压到与元信息一致）。 */
const badgeSx = { height: 18, fontSize: "0.6rem", ml: 0.5 };

/** 强调行的语义调色板键；`null` 表示非强调行。 */
type AccentKey = "primary" | "error" | "warning" | null;

/**
 * 判定行的强调类别。`missing` / `deleted` 优先于 `upgradeable`（与标记派生规则一致）。
 */
function accentKey(item: UnifiedVideoItem): AccentKey {
  if (item.missing || item.status === "deleted") return "error";
  if (item.upgradeable) return "primary";
  if (item.status === "retrying") return "warning";
  return null;
}

/**
 * 行首 3px 色条的颜色语义键。
 *
 * 非强调行用 `transparent` 占位 —— 直接省略边框会让行宽在有无强调之间跳动。
 */
function accentColor(item: UnifiedVideoItem): string {
  const key = accentKey(item);
  return key ? `${key}.main` : "transparent";
}

/**
 * 强调行的**极淡底色**（与色条配套，decisions §8）。
 *
 * 用 `alpha(..., 0.06)` 而非固定色值，使亮/暗色主题各自解析出合适的浅色。
 */
function accentTint(item: UnifiedVideoItem, theme: Theme): string {
  const key = accentKey(item);
  return key ? alpha(theme.palette[key].main, 0.06) : "transparent";
}

interface DetailPanelProps {
  subscription: Subscription | null;
  /** **全局去重后**的下载记录（含其它订阅的归属，用于判定「已在其它订阅下载」） */
  records: DownloadRecord[];
  queueTasks: DownloadTask[];
  /** 磁盘上已缺失的文件路径集合 */
  missingPaths?: Set<string>;
  error?: string | null;
  progressMap?: Map<string, DownloadProgress>;
  onPauseDownload: (videoUrl: string) => void;
  onResumeDownload: (taskId: string) => void;
  onCancelDownload: (videoUrl: string) => void;
  /** 按订阅当前画质重下某条记录（升级 / 重新下载 / 重试共用） */
  onRedownload: (recordId: string) => void;
  /** 修改订阅的画质 preset */
  onUpdateQuality: (id: string, quality: string) => void;
}

/** Right-side detail panel showing channel info and download records. */
export default function DetailPanel({
  subscription,
  records,
  queueTasks,
  missingPaths,
  error,
  progressMap,
  onPauseDownload,
  onCancelDownload,
  onRedownload,
  onUpdateQuality,
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
          } catch (e) {
            // 不能静默中断：后端解析失败时只会表现为「列表少了一截」，
            // 而后端早已改成解析失败即报错，这里吞掉就等于让那个改动失效。
            // 已加载的条目保留，同时把错误显示出来。
            if (currentRequestId === requestIdRef.current) setLoadError(String(e));
            break;
          }
        }
      }
    };

    loadData();
  }, [subscription?.id]);

  const isDead = subscription?.health_status === "dead";

  // 合并频道视频、下载记录、队列任务为统一列表，并派生「可升级 / 文件缺失 /
  // 已在其它订阅下载」三个正交标记。records 是全局记录，靠 subscriptionId 区分归属。
  const unifiedItems = useMemo(() => {
    try {
      return buildUnifiedVideoList({
        videos: videoList,
        records,
        tasks: queueTasks,
        qualityPreset: subscription?.quality_preset,
        missingPaths,
        subscriptionId: subscription?.id,
      });
    } catch (e) {
      console.error("[DetailPanel] buildUnifiedVideoList failed:", e);
      return [];
    }
  }, [
    videoList,
    records,
    queueTasks,
    subscription?.quality_preset,
    subscription?.id,
    missingPaths,
  ]);

  // 已完成数从 records 派生（唯一真相源是下载记录）。因为现在收到的是**全局**记录，
  // 必须按本订阅过滤 —— 口径：本订阅仍处于 completed 的记录条数。
  const completedCount = useMemo(
    () =>
      records.filter(
        (r) => r.subscription_id === subscription?.id && r.status === "completed",
      ).length,
    [records, subscription?.id],
  );

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
            {/* 订阅级画质设置入口：复用既有 update_subscription_quality 命令。
                界面只给 best / 1080p / 720p / 480p / audio 五档；若订阅现值不在其中
                （API 或手改文件产生），把它一并列出来以免 Select 渲染为空。 */}
            <Select
              value={subscription.quality_preset}
              onChange={(e) => onUpdateQuality(subscription.id, e.target.value)}
              size="small"
              variant="outlined"
              title="下载画质"
              aria-label="下载画质"
              sx={{
                height: 20,
                fontSize: "0.7rem",
                "& .MuiSelect-select": {
                  py: 0,
                  pl: 0.75,
                  pr: "20px !important",
                  fontSize: "0.7rem",
                },
                "& .MuiSelect-icon": { fontSize: 14, right: 2 },
              }}
            >
              {Array.from(
                new Set<string>([...QUALITY_OPTIONS, subscription.quality_preset]),
              ).map((q) => (
                <MenuItem key={q} value={q} sx={{ fontSize: "0.75rem" }}>
                  {qualityLabel(q)}
                </MenuItem>
              ))}
            </Select>
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
              已完成: {completedCount} 个
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
                            // 行首 3px 色条：可升级 / 缺失·已删除 / 重试中
                            borderLeft: "3px solid",
                            borderLeftColor: accentColor(item),
                            // 与色条配套的极淡底色（亮/暗主题各自解析）
                            bgcolor: (theme) => accentTint(item, theme),
                            opacity: item.status === "deleted" ? 0.5 : 1,
                          }}
                        >
                          {/* 第 1 列：状态图标（垂直居中） */}
                          <Box sx={{ width: 40, flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center" }}>
                            {item.status === "new" && <VideoIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                            {item.status === "downloading" && <DownloadingIcon sx={{ fontSize: 18, color: "info.main" }} />}
                            {item.status === "retrying" && <RetryingIcon sx={{ fontSize: 18, color: "warning.main" }} />}
                            {item.status === "paused" && <PauseIcon sx={{ fontSize: 18, color: "warning.main" }} />}
                            {item.status === "waiting" && <WaitingIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                            {item.status === "completed" && <SuccessIcon sx={{ fontSize: 18, color: "success.main" }} />}
                            {item.status === "failed" && <ErrorIcon sx={{ fontSize: 18, color: "error.main" }} />}
                            {item.status === "cancelled" && <CancelIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                            {item.status === "deleted" && <DeletedIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
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
                              sx={{
                                fontSize: "0.8rem",
                                lineHeight: 1.3,
                                ...(item.status === "deleted" && { textDecoration: "line-through" }),
                              }}
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
                                      {item.status === "retrying" && (
                                        <>
                                          <Box component="span" sx={{ color: "warning.main", fontWeight: 500 }}>
                                            重试中
                                          </Box>
                                          {` (${item.downloadInfo?.retry_count ?? 0}/${MAX_RETRY_ATTEMPTS})`}
                                          {item.queueTask?.next_retry_at && (
                                            <>
                                              {" "}
                                              <RetryCountdown nextRetryAt={item.queueTask.next_retry_at} />
                                            </>
                                          )}
                                        </>
                                      )}
                                      {item.status === "deleted" && (
                                        <>
                                          <Box component="span" sx={{ color: "text.disabled", fontWeight: 500 }}>已删除</Box>
                                          {date && ` · ${date}`}
                                          {!item.channelInfo && item.downloadInfo &&
                                            ` · ${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN")}`}
                                        </>
                                      )}
                                    </>
                                  );
                                })()}
                              </Typography>
                              {/* 徽标（chip）：可升级 / 文件缺失 / 已在其它订阅下载。
                                  缺失与可升级互斥，缺失优先（由派生规则保证）。 */}
                              {item.upgradeable && (
                                <Chip
                                  label={`可升级 ${qualityLabel(subscription.quality_preset)}`}
                                  size="small"
                                  color="primary"
                                  variant="outlined"
                                  sx={badgeSx}
                                />
                              )}
                              {item.missing && (
                                <Chip label="文件缺失" size="small" color="error" sx={badgeSx} />
                              )}
                              {item.downloadedElsewhere && (
                                <Chip
                                  label="已在其它订阅下载"
                                  size="small"
                                  variant="outlined"
                                  sx={badgeSx}
                                />
                              )}
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
                                  data-testid="download-progress-fill"
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
                            {/* 升级：仅「可升级」时出现（已在其它订阅下载的行不提供） */}
                            {item.upgradeable && item.downloadInfo && !item.downloadedElsewhere && (
                              <IconButton
                                size="small"
                                onClick={() => onRedownload(item.downloadInfo!.id)}
                                title={`升级到 ${qualityLabel(subscription.quality_preset)}`}
                                sx={{ color: "primary.main" }}
                              >
                                <UpgradeIcon sx={{ fontSize: 18 }} />
                              </IconButton>
                            )}
                            {/* 重新下载：「文件缺失」与「已删除」共用同一入口 */}
                            {(item.missing || item.status === "deleted") &&
                              item.downloadInfo &&
                              !item.downloadedElsewhere && (
                                <IconButton
                                  size="small"
                                  onClick={() => onRedownload(item.downloadInfo!.id)}
                                  title="重新下载"
                                  sx={{ color: "error.main" }}
                                >
                                  <RedownloadIcon sx={{ fontSize: 18 }} />
                                </IconButton>
                              )}
                            {item.queueTask && (item.status === "downloading" || item.status === "retrying") && (
                              <IconButton size="small" onClick={() => onPauseDownload(item.url)} title="暂停">
                                <PauseIcon sx={{ fontSize: 18 }} />
                              </IconButton>
                            )}
                            {item.queueTask && (item.status === "downloading" || item.status === "retrying" || item.status === "waiting" || item.status === "paused") && (
                              <IconButton size="small" onClick={() => onCancelDownload(item.url)} title="取消" sx={{ color: "error.main" }}>
                                <CancelIcon sx={{ fontSize: 18 }} />
                              </IconButton>
                            )}
                            {/* 失败重试：只重下这一条记录（不再是「重查整个订阅」） */}
                            {item.status === "failed" && item.downloadInfo && !item.downloadedElsewhere && (
                              <IconButton
                                size="small"
                                onClick={() => onRedownload(item.downloadInfo!.id)}
                                title="重试"
                                sx={{ color: "warning.main" }}
                              >
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
