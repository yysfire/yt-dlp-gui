use tauri::State;

use crate::models::Subscription;
use crate::services::{StorageService, YtDlpService};
use crate::utils::AppError;
use crate::AppContext;

/// Adds a new subscription by parsing the channel URL and saving it.
///
/// Returns an error if the URL is already subscribed.
#[tauri::command]
pub async fn add_subscription(
    url: String,
    state: State<'_, AppContext>,
) -> Result<Subscription, String> {
    // Load current subscriptions to check for duplicates
    let mut subs = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| e.to_string())?;

    // Check for duplicate URL
    if subs.iter().any(|s| s.url == url) {
        return Err(AppError::Duplicate(format!(
            "Already subscribed to: {}",
            url
        ))
        .to_string());
    }

    // Get yt-dlp path, proxy, and cookie_file from settings
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    let yt_dlp_path = settings.yt_dlp_path.clone();
    let proxy = Some(settings.proxy_url.clone());
    let cookie_file = Some(settings.cookie_file.clone());

    // Parse channel info via yt-dlp
    let channel_info = YtDlpService::parse_channel_info(&yt_dlp_path, &proxy, &cookie_file, &url)
        .map_err(|e| e.to_string())?;

    let subscription = Subscription::new(
        url,
        channel_info.platform,
        channel_info.channel_name,
        channel_info.channel_avatar_url,
    );

    subs.push(subscription.clone());

    StorageService::save_subscriptions(&state.data_dir, &subs)
        .map_err(|e| e.to_string())?;

    log::info!("Added subscription: {}", subscription.channel_name);
    Ok(subscription)
}

/// Deletes a subscription and all associated download records.
#[tauri::command]
pub async fn delete_subscription(
    id: String,
    state: State<'_, AppContext>,
) -> Result<(), String> {
    let mut subs = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| e.to_string())?;

    let index = subs
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound(format!("Subscription not found: {}", id)))
        .map_err(|e| e.to_string())?;

    let removed = subs.remove(index);

    StorageService::save_subscriptions(&state.data_dir, &subs)
        .map_err(|e| e.to_string())?;

    // Cascade delete download records for this subscription
    let mut records = StorageService::load_download_records(&state.data_dir)
        .map_err(|e| e.to_string())?;
    records.retain(|r| r.subscription_id != id);
    StorageService::save_download_records(&state.data_dir, &records)
        .map_err(|e| e.to_string())?;

    log::info!("Deleted subscription: {}", removed.channel_name);
    Ok(())
}

/// Returns all subscriptions.
#[tauri::command]
pub async fn get_subscriptions(
    state: State<'_, AppContext>,
) -> Result<Vec<Subscription>, String> {
    StorageService::load_subscriptions(&state.data_dir).map_err(|e| e.to_string())
}

/// Toggles the paused state of a subscription.
#[tauri::command]
pub async fn toggle_subscription_pause(
    id: String,
    state: State<'_, AppContext>,
) -> Result<Subscription, String> {
    let mut subs = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| e.to_string())?;

    let sub = subs
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound(format!("Subscription not found: {}", id)))
        .map_err(|e| e.to_string())?;

    sub.paused = !sub.paused;
    let updated = sub.clone();

    StorageService::save_subscriptions(&state.data_dir, &subs)
        .map_err(|e| e.to_string())?;

    log::info!(
        "Toggled subscription pause: {} -> {}",
        updated.channel_name,
        updated.paused
    );
    Ok(updated)
}

/// Updates the quality preset for a subscription.
#[tauri::command]
pub async fn update_subscription_quality(
    id: String,
    quality_preset: String,
    state: State<'_, AppContext>,
) -> Result<Subscription, String> {
    let mut subs = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| e.to_string())?;

    let sub = subs
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound(format!("Subscription not found: {}", id)))
        .map_err(|e| e.to_string())?;

    sub.quality_preset = quality_preset;
    let updated = sub.clone();

    StorageService::save_subscriptions(&state.data_dir, &subs)
        .map_err(|e| e.to_string())?;

    log::info!(
        "Updated quality for {}: {}",
        updated.channel_name,
        updated.quality_preset
    );
    Ok(updated)
}
