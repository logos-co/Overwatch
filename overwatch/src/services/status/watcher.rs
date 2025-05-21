use std::time::Duration;

use tokio::sync::watch::Ref;

use crate::services::status::{service_status::ServiceStatus, Receiver};

/// Watcher for the [`ServiceStatus`] updates.
#[derive(Debug, Clone)]
pub struct StatusWatcher(Receiver);

impl StatusWatcher {
    /// Create a new [`StatusWatcher`].
    #[must_use]
    pub const fn new(receiver: Receiver) -> Self {
        Self(receiver)
    }
}

impl StatusWatcher {
    /// Wait for a new [`ServiceStatus`] message.
    ///
    /// # Errors
    ///
    /// If the message is not received within the specified timeout period.
    pub async fn wait_for(
        &mut self,
        status: ServiceStatus,
        timeout_duration: Option<Duration>,
    ) -> Result<ServiceStatus, ServiceStatus> {
        let current = *self.current();
        if status == current {
            return Ok(current);
        }
        let timeout_duration = timeout_duration.unwrap_or_else(|| Duration::from_secs(u64::MAX));
        tokio::time::timeout(timeout_duration, self.0.wait_for(|s| s == &status))
            .await
            .map(|r| r.map(|s| *s).map_err(|_| current))
            .unwrap_or(Err(current))
    }

    #[must_use]
    pub fn current(&self) -> Ref<'_, ServiceStatus> {
        self.0.borrow()
    }
}
