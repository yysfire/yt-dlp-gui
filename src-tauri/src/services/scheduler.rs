use std::sync::{Arc, Mutex};

/// Service for managing the periodic subscription check scheduler.
pub struct SchedulerService;

impl SchedulerService {
    /// Starts a background task that invokes `callback` every `interval_mins` minutes.
    ///
    /// Returns a `JoinHandle` that can be used to abort the task via `stop()`.
    /// The callback is wrapped in an `Arc<Mutex<>>` so it can be called from the spawned task.
    pub fn start<F>(
        interval_mins: u32,
        callback: F,
    ) -> tokio::task::JoinHandle<()>
    where
        F: Fn() + Send + 'static,
    {
        let cb = Arc::new(Mutex::new(Some(callback)));

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs((interval_mins as u64) * 60),
            );

            // Skip the immediate first tick — wait for the first interval
            interval.tick().await;

            loop {
                interval.tick().await;
                if let Some(ref cb) = *cb.lock().unwrap() {
                    cb();
                }
            }
        })
    }

    /// Stops a running scheduler by aborting its task handle.
    pub fn stop(handle: tokio::task::JoinHandle<()>) {
        handle.abort();
    }
}
