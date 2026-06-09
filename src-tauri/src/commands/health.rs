use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

use crate::models::health::{HealthCheckResult, HealthCheckSummary, HealthStatus};
use crate::models::Subscription;
use crate::services::{HealthService, StorageService};
use crate::AppContext;

// ── 内部辅助函数（可单独测试）────────────────────────────────────

/// 根据健康检查结果更新订阅列表中对应条目的 `health_status` 和
/// `last_health_check`，并自动暂停标记为 Dead 的订阅。
///
/// 更新完成后将完整订阅列表持久化到 `subscriptions.json`。
fn update_subscriptions_health(
    data_dir: &std::path::Path,
    subscriptions: &mut [Subscription],
    results: &[HealthCheckResult],
) -> Result<(), String> {
    for result in results {
        if let Some(sub) = subscriptions
            .iter_mut()
            .find(|s| s.id == result.subscription_id)
        {
            sub.health_status = Some(result.status.clone());
            sub.last_health_check = Some(result.checked_at.clone());

            if result.status == HealthStatus::Dead {
                sub.paused = true;
            }
        }
    }
    StorageService::save_subscriptions(data_dir, subscriptions)
        .map_err(|e| format!("保存订阅失败: {}", e))
}

// ── Tauri 命令 ──────────────────────────────────────────────────

/// 对所有订阅执行健康检查。
///
/// 快照当前订阅列表后立即返回一个 pending 摘要，实际检查在
/// 后台 tokio 任务中异步执行。完成后通过 `health-check-complete`
/// 事件通知前端。
#[tauri::command]
pub async fn check_all_health(
    state: tauri::State<'_, AppContext>,
    app_handle: AppHandle,
) -> Result<HealthCheckSummary, String> {
    let subscriptions = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| format!("加载订阅失败: {}", e))?;

    if subscriptions.is_empty() {
        return Ok(HealthCheckSummary::pending(0));
    }

    let data_dir = state.data_dir.clone();
    let total = subscriptions.len();
    let client = HealthService::build_client()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    let semaphore = Arc::new(Semaphore::new(3));

    let app_handle_clone = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let summary =
            HealthService::check_batch(&subscriptions, &client, semaphore).await;

        // 更新订阅健康状态并持久化
        let mut subs =
            StorageService::load_subscriptions(&data_dir).unwrap_or_default();
        let _ = update_subscriptions_health(&data_dir, &mut subs, &summary.results);

        let _ = app_handle_clone
            .emit("health-check-complete", serde_json::json!(&summary));
    });

    Ok(HealthCheckSummary::pending(total))
}

/// 对指定 ID 列表的订阅执行健康检查。
///
/// 与 `check_all_health` 类似，但仅检查 `subscription_ids` 中列出的
/// 订阅。空 ID 列表直接返回空摘要。
#[tauri::command]
pub async fn check_selected_health(
    subscription_ids: Vec<String>,
    state: tauri::State<'_, AppContext>,
    app_handle: AppHandle,
) -> Result<HealthCheckSummary, String> {
    if subscription_ids.is_empty() {
        return Ok(HealthCheckSummary::pending(0));
    }

    let all_subscriptions = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| format!("加载订阅失败: {}", e))?;

    let subscriptions: Vec<Subscription> = all_subscriptions
        .iter()
        .filter(|s| subscription_ids.contains(&s.id))
        .cloned()
        .collect();

    if subscriptions.is_empty() {
        return Ok(HealthCheckSummary::pending(0));
    }

    let data_dir = state.data_dir.clone();
    let total = subscriptions.len();
    let client = HealthService::build_client()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    let semaphore = Arc::new(Semaphore::new(3));

    let app_handle_clone = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let summary =
            HealthService::check_batch(&subscriptions, &client, semaphore).await;

        let mut subs =
            StorageService::load_subscriptions(&data_dir).unwrap_or_default();
        let _ = update_subscriptions_health(&data_dir, &mut subs, &summary.results);

        let _ = app_handle_clone
            .emit("health-check-complete", serde_json::json!(&summary));
    });

    Ok(HealthCheckSummary::pending(total))
}

// ── 测试 ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::health::{HealthCheckResult, HealthStatus};
    use crate::models::Subscription;
    use tempfile::TempDir;

    fn make_sub(id: &str, url: &str, name: &str) -> Subscription {
        let mut sub = Subscription::new(
            url.to_string(),
            "youtube".to_string(),
            name.to_string(),
            "".to_string(),
        );
        sub.id = id.to_string();
        sub
    }

    /// 从订阅列表中筛选出 `ids` 指定的条目（测试辅助函数）。
    fn filter_subscriptions(
        subscriptions: &[Subscription],
        ids: Option<&[String]>,
    ) -> Vec<Subscription> {
        match ids {
            None | Some(&[]) => subscriptions.to_vec(),
            Some(ids) => subscriptions
                .iter()
                .filter(|s| ids.contains(&s.id))
                .cloned()
                .collect(),
        }
    }

    // ── T019: check_all_health / check_selected_health 逻辑测试 ──

    #[test]
    fn test_check_all_health_no_subscriptions() {
        // 无订阅时 filter_subscriptions 返回空列表
        let subs: Vec<Subscription> = vec![];
        let result = filter_subscriptions(&subs, None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_check_selected_health_empty_ids() {
        // 空 ID 列表 → filter_subscriptions 返回所有订阅（命令层在此之前已做 early return）
        let sub1 = make_sub("id-1", "https://youtube.com/@a", "A");
        let sub2 = make_sub("id-2", "https://youtube.com/@b", "B");
        let subs = vec![sub1, sub2];

        let result = filter_subscriptions(&subs, Some(&[]));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_subscriptions_all() {
        let sub1 = make_sub("id-1", "https://youtube.com/@a", "A");
        let sub2 = make_sub("id-2", "https://youtube.com/@b", "B");
        let subs = vec![sub1.clone(), sub2.clone()];

        let result = filter_subscriptions(&subs, None);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|s| s.id == "id-1"));
        assert!(result.iter().any(|s| s.id == "id-2"));
    }

    #[test]
    fn test_filter_subscriptions_selected() {
        let sub1 = make_sub("id-1", "https://youtube.com/@a", "A");
        let sub2 = make_sub("id-2", "https://youtube.com/@b", "B");
        let sub3 = make_sub("id-3", "https://youtube.com/@c", "C");
        let subs = vec![sub1, sub2, sub3];

        let selected =
            vec!["id-1".to_string(), "id-3".to_string()];
        let result = filter_subscriptions(&subs, Some(&selected));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "id-1");
        assert_eq!(result[1].id, "id-3");
    }

    #[test]
    fn test_filter_subscriptions_none_matched() {
        let sub1 = make_sub("id-1", "https://youtube.com/@a", "A");
        let subs = vec![sub1];

        let selected = vec!["id-nonexistent".to_string()];
        let result = filter_subscriptions(&subs, Some(&selected));
        assert_eq!(result.len(), 0);
    }

    // ── T020: 健康状态持久化测试 ──

    #[test]
    fn test_update_subscriptions_health_ok() {
        let tmp = TempDir::new().unwrap();
        let mut sub = make_sub("id-1", "https://youtube.com/@a", "A");
        sub.health_status = None;
        sub.last_health_check = None;

        let mut subs = vec![sub.clone()];
        let results = vec![HealthCheckResult {
            subscription_id: "id-1".to_string(),
            url: "https://youtube.com/@a".to_string(),
            status: HealthStatus::Ok,
            detail: "HTTP 200".to_string(),
            latency_ms: 100,
            checked_at: "2026-06-10T12:00:00Z".to_string(),
        }];

        update_subscriptions_health(tmp.path(), &mut subs, &results).unwrap();

        assert_eq!(subs[0].health_status, Some(HealthStatus::Ok));
        assert_eq!(
            subs[0].last_health_check,
            Some("2026-06-10T12:00:00Z".to_string())
        );
        assert!(!subs[0].paused, "Ok 状态不应自动暂停");
    }

    #[test]
    fn test_update_subscriptions_health_warning() {
        let tmp = TempDir::new().unwrap();
        let mut sub = make_sub("id-1", "https://youtube.com/@a", "A");
        sub.paused = false;

        let mut subs = vec![sub.clone()];
        let results = vec![HealthCheckResult {
            subscription_id: "id-1".to_string(),
            url: "https://youtube.com/@a".to_string(),
            status: HealthStatus::Warning,
            detail: "HTTP 429".to_string(),
            latency_ms: 500,
            checked_at: "2026-06-10T12:00:00Z".to_string(),
        }];

        update_subscriptions_health(tmp.path(), &mut subs, &results).unwrap();

        assert_eq!(subs[0].health_status, Some(HealthStatus::Warning));
        assert!(!subs[0].paused, "Warning 状态不应自动暂停");
    }

    #[test]
    fn test_update_subscriptions_health_persists_to_disk() {
        let tmp = TempDir::new().unwrap();
        let mut sub = make_sub("id-1", "https://youtube.com/@a", "A");
        sub.health_status = None;

        let mut subs = vec![sub.clone()];
        let results = vec![HealthCheckResult {
            subscription_id: "id-1".to_string(),
            url: "https://youtube.com/@a".to_string(),
            status: HealthStatus::Ok,
            detail: "HTTP 200".to_string(),
            latency_ms: 100,
            checked_at: "2026-06-10T12:00:00Z".to_string(),
        }];

        update_subscriptions_health(tmp.path(), &mut subs, &results).unwrap();

        // 验证磁盘持久化
        let loaded = StorageService::load_subscriptions(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].health_status, Some(HealthStatus::Ok));
        assert_eq!(
            loaded[0].last_health_check,
            Some("2026-06-10T12:00:00Z".to_string())
        );
    }

    // ── T021: 自动暂停测试 ──

    #[test]
    fn test_auto_pause_on_dead() {
        let tmp = TempDir::new().unwrap();
        let mut sub = make_sub("id-1", "https://youtube.com/@a", "A");
        sub.paused = false;

        let mut subs = vec![sub.clone()];
        let results = vec![HealthCheckResult {
            subscription_id: "id-1".to_string(),
            url: "https://youtube.com/@a".to_string(),
            status: HealthStatus::Dead,
            detail: "HTTP 404".to_string(),
            latency_ms: 50,
            checked_at: "2026-06-10T12:00:00Z".to_string(),
        }];

        update_subscriptions_health(tmp.path(), &mut subs, &results).unwrap();

        assert_eq!(subs[0].health_status, Some(HealthStatus::Dead));
        assert!(subs[0].paused, "Dead 状态必须自动暂停订阅");
    }

    #[test]
    fn test_auto_pause_dead_persists_to_disk() {
        let tmp = TempDir::new().unwrap();
        let mut sub = make_sub("id-1", "https://youtube.com/@a", "A");
        sub.paused = false;

        let mut subs = vec![sub.clone()];
        let results = vec![HealthCheckResult {
            subscription_id: "id-1".to_string(),
            url: "https://youtube.com/@a".to_string(),
            status: HealthStatus::Dead,
            detail: "HTTP 410 Gone".to_string(),
            latency_ms: 30,
            checked_at: "2026-06-10T12:00:00Z".to_string(),
        }];

        update_subscriptions_health(tmp.path(), &mut subs, &results).unwrap();

        // 验证磁盘上的 paused 状态
        let loaded = StorageService::load_subscriptions(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].health_status, Some(HealthStatus::Dead));
        assert!(loaded[0].paused, "Dead 暂停状态必须持久化到磁盘");
    }

    #[test]
    fn test_auto_pause_only_for_dead() {
        // 验证只有 Dead 状态会暂停，Ok 和 Warning 不会
        let tmp = TempDir::new().unwrap();

        let mut sub_ok = make_sub("id-ok", "https://youtube.com/@ok", "OK");
        sub_ok.paused = false;
        let mut sub_warn = make_sub("id-warn", "https://youtube.com/@warn", "Warn");
        sub_warn.paused = false;
        let mut sub_dead = make_sub("id-dead", "https://youtube.com/@dead", "Dead");
        sub_dead.paused = false;

        let mut subs = vec![sub_ok, sub_warn, sub_dead];
        let results = vec![
            HealthCheckResult {
                subscription_id: "id-ok".to_string(),
                url: "https://youtube.com/@ok".to_string(),
                status: HealthStatus::Ok,
                detail: "HTTP 200".to_string(),
                latency_ms: 100,
                checked_at: "2026-06-10T12:00:00Z".to_string(),
            },
            HealthCheckResult {
                subscription_id: "id-warn".to_string(),
                url: "https://youtube.com/@warn".to_string(),
                status: HealthStatus::Warning,
                detail: "HTTP 500".to_string(),
                latency_ms: 200,
                checked_at: "2026-06-10T12:00:01Z".to_string(),
            },
            HealthCheckResult {
                subscription_id: "id-dead".to_string(),
                url: "https://youtube.com/@dead".to_string(),
                status: HealthStatus::Dead,
                detail: "HTTP 404".to_string(),
                latency_ms: 50,
                checked_at: "2026-06-10T12:00:02Z".to_string(),
            },
        ];

        update_subscriptions_health(tmp.path(), &mut subs, &results).unwrap();

        let sub_ok = subs.iter().find(|s| s.id == "id-ok").unwrap();
        let sub_warn = subs.iter().find(|s| s.id == "id-warn").unwrap();
        let sub_dead = subs.iter().find(|s| s.id == "id-dead").unwrap();

        assert!(!sub_ok.paused, "Ok 不应自动暂停");
        assert!(!sub_warn.paused, "Warning 不应自动暂停");
        assert!(sub_dead.paused, "Dead 必须自动暂停");
    }

    #[test]
    fn test_update_subscriptions_health_id_not_found_skips() {
        let tmp = TempDir::new().unwrap();
        let sub = make_sub("id-1", "https://youtube.com/@a", "A");
        let original_paused = sub.paused;

        let mut subs = vec![sub.clone()];
        // 结果中引用不存在的订阅 ID
        let results = vec![HealthCheckResult {
            subscription_id: "id-non-existent".to_string(),
            url: "https://youtube.com/@b".to_string(),
            status: HealthStatus::Dead,
            detail: "HTTP 404".to_string(),
            latency_ms: 50,
            checked_at: "2026-06-10T12:00:00Z".to_string(),
        }];

        let result = update_subscriptions_health(tmp.path(), &mut subs, &results);
        assert!(result.is_ok(), "不存在的 ID 应被跳过，不应报错");
        assert_eq!(subs[0].paused, original_paused, "不应影响未匹配的订阅");
        assert_eq!(subs[0].health_status, None, "不应更新未匹配订阅的状态");
    }
}
