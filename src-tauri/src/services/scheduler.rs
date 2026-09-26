//! 后台周期任务：自动检查订阅 + 文件状态同步。
//!
//! 由 `lib.rs` 的 setup 装配（[`spawn_scheduler`] + [`spawn_file_sync`]）。任务句柄不保存：
//! 应用运行期间它们常驻，当前不需要启停能力。
//!
//! 两个循环都在**每一轮重新从磁盘读取设置**，因此 yt-dlp 路径、代理、Cookie、
//! 下载目录、检查间隔的改动会在下一轮生效，无需重启。

use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::services::StorageService;

/// 一轮检查之后要等多久。
#[derive(Debug, PartialEq, Eq)]
enum WaitPlan {
    /// 间隔为 0：手动模式，只在设置变更时唤醒，不做周期检查。
    Manual,
    /// 定时模式。
    Timed(Duration),
}

fn wait_plan(interval_mins: u32) -> WaitPlan {
    if interval_mins == 0 {
        WaitPlan::Manual
    } else {
        WaitPlan::Timed(Duration::from_secs((interval_mins as u64) * 60))
    }
}

/// 自动调度是否应该执行本轮检查（`scheduler_paused` 由托盘菜单管理）。
fn should_run_round(scheduler_paused: bool) -> bool {
    !scheduler_paused
}

/// 启动自动检查调度循环（立即返回，不阻塞 setup）。
///
/// `notify` 是设置变更通知的接收端；对应的发送端存放在 `AppContext.scheduler_notify`，
/// **必须保持存活**，否则 `changed()` 会立刻返回 `Err` 导致忙循环。
pub fn spawn_scheduler(
    data_dir: PathBuf,
    app_handle: AppHandle,
    notify: watch::Receiver<()>,
) {
    let _ = tauri::async_runtime::spawn(async move {
        run_scheduler_loop(data_dir, app_handle, notify).await;
    });
}

async fn run_scheduler_loop(
    data_dir: PathBuf,
    app_handle: AppHandle,
    mut notify: watch::Receiver<()>,
) {
    // 首轮语义：定时模式先等满一个间隔（这里必须是裸 sleep、不能进 select!，
    // 否则启动后的一次设置变更会提前触发首轮）；手动模式立即检查一轮。
    if let WaitPlan::Timed(delay) =
        wait_plan(StorageService::load_settings(&data_dir).check_interval_minutes)
    {
        tokio::time::sleep(delay).await;
    }

    loop {
        // 每轮重读设置：让 yt-dlp 路径 / 代理 / Cookie / 下载目录的改动即时生效，
        // 同时拿到最新的间隔与暂停标志。
        let settings = StorageService::load_settings(&data_dir);
        let interval_mins = settings.check_interval_minutes;

        if should_run_round(settings.scheduler_paused) {
            log::info!("Scheduler: checking subscriptions...");
            let _ = crate::commands::download::run_check_round(&data_dir, &app_handle).await;
        } else {
            log::info!("Scheduler: paused, skipping this round");
        }

        // 等待：手动模式只等设置变更通知；定时模式让睡眠与通知竞争，
        // 使间隔变更 / 取消暂停能立即唤醒（`watch::Receiver::changed` 是 cancel-safe 的）。
        match wait_plan(interval_mins) {
            WaitPlan::Manual => {
                let _ = notify.changed().await;
            }
            WaitPlan::Timed(delay) => {
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = notify.changed() => {}
                }
            }
        }
    }
}

/// 启动周期性的文件状态同步：每 5 分钟检查一次已完成记录的文件是否还在（立即返回）。
pub fn spawn_file_sync(data_dir: PathBuf, app_handle: AppHandle) {
    let _ = tauri::async_runtime::spawn(async move {
        let interval = Duration::from_secs(5 * 60);
        // 跳过启动时的首个立即 tick
        tokio::time::sleep(interval).await;

        loop {
            let records = StorageService::load_download_records(&data_dir).unwrap_or_default();
            let file_paths: Vec<String> = records
                .iter()
                .filter(|r| r.status == "completed" && !r.file_path.is_empty())
                .map(|r| r.file_path.clone())
                .collect();
            let results = crate::services::file_manager::check_files_exist(&file_paths).await;
            let _ = app_handle.emit("file-sync-complete", &results);
            tokio::time::sleep(interval).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_plan_manual_when_zero() {
        // 0 = 手动模式（只在设置变更时唤醒），是设置里四档频率之一
        assert_eq!(wait_plan(0), WaitPlan::Manual);
    }

    #[test]
    fn wait_plan_timed_when_positive() {
        assert_eq!(wait_plan(60), WaitPlan::Timed(Duration::from_secs(3600)));
    }

    #[test]
    fn wait_plan_scales_all_frequency_options() {
        // 分钟 → 秒的换算，覆盖设置里提供的全部档位
        assert_eq!(wait_plan(1), WaitPlan::Timed(Duration::from_secs(60)));
        assert_eq!(wait_plan(30), WaitPlan::Timed(Duration::from_secs(1800)));
        assert_eq!(
            wait_plan(1440),
            WaitPlan::Timed(Duration::from_secs(86400))
        );
    }

    #[test]
    fn should_run_round_true_when_active() {
        assert!(should_run_round(false));
    }

    #[test]
    fn should_run_round_false_when_paused() {
        // 极性写反会导致「永不检查」，静默且致命
        assert!(!should_run_round(true));
    }
}
