use std::sync::Arc;
use std::time::Duration;

use crate::error::ModalError;

/// How often ephemeral objects send heartbeats to keep themselves alive.
/// Matches Go SDK's `ephemeralObjectHeartbeatSleep` (300s).
pub const EPHEMERAL_OBJECT_HEARTBEAT_SLEEP: Duration = Duration::from_secs(300);

/// Start an ephemeral heartbeat loop that runs until the notify is triggered.
///
/// This is the Rust equivalent of Go's `startEphemeralHeartbeat` in `ephemeral.go`.
/// The heartbeat function is called every `EPHEMERAL_OBJECT_HEARTBEAT_SLEEP` interval.
/// Errors from the heartbeat function are silently ignored (matching Go behavior).
/// Cancel the heartbeat by calling `notify.notify_one()`.
pub fn start_ephemeral_heartbeat<F>(notify: Arc<tokio::sync::Notify>, heartbeat_fn: F)
where
    F: Fn() -> Result<(), ModalError> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = notify.notified() => {
                    return;
                }
                _ = tokio::time::sleep(EPHEMERAL_OBJECT_HEARTBEAT_SLEEP) => {
                    let _ = heartbeat_fn();
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_start_ephemeral_heartbeat_cancel() {
        let notify = Arc::new(tokio::sync::Notify::new());
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        start_ephemeral_heartbeat(notify.clone(), move || {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        // Cancel immediately — heartbeat should not have fired
        notify.notify_one();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn test_start_ephemeral_heartbeat_fires() {
        let notify = Arc::new(tokio::sync::Notify::new());
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        start_ephemeral_heartbeat(notify.clone(), move || {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        // Sleep auto-advances time in paused mode and yields to spawned tasks
        tokio::time::sleep(EPHEMERAL_OBJECT_HEARTBEAT_SLEEP + Duration::from_millis(10)).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        tokio::time::sleep(EPHEMERAL_OBJECT_HEARTBEAT_SLEEP + Duration::from_millis(10)).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 2);

        notify.notify_one();
    }

    #[tokio::test(start_paused = true)]
    async fn test_start_ephemeral_heartbeat_ignores_errors() {
        let notify = Arc::new(tokio::sync::Notify::new());
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        start_ephemeral_heartbeat(notify.clone(), move || {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            Err(ModalError::Other("heartbeat failed".to_string()))
        });

        // Even with errors, the loop keeps running
        tokio::time::sleep(EPHEMERAL_OBJECT_HEARTBEAT_SLEEP + Duration::from_millis(10)).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        tokio::time::sleep(EPHEMERAL_OBJECT_HEARTBEAT_SLEEP + Duration::from_millis(10)).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 2);

        notify.notify_one();
    }
}
