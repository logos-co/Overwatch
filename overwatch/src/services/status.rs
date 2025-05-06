use std::{default::Default, sync::Arc, time::Duration};

use tokio::sync::{watch, watch::Ref};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ServiceStatus {
    Uninitialized,
    Running,
    Stopped,
}

pub struct StatusUpdater(watch::Sender<ServiceStatus>);

impl StatusUpdater {
    /// Send a status update message to the associated service.
    ///
    /// # Panics
    ///
    /// If the message cannot be sent to the target service.
    pub fn update(&self, status: ServiceStatus) {
        self.0
            .send(status)
            .expect("Overwatch always maintain an open watcher, send should always succeed");
    }
}

/// Watcher for the service status.
/// TODO
#[derive(Debug, Clone)]
pub struct StatusWatcher(watch::Receiver<ServiceStatus>);

impl StatusWatcher {
    /// Wait for a [`ServiceStatus`] message.
    ///
    /// # Errors
    ///
    /// If the message is not received within the specified timeout period.
    pub async fn wait_for(
        &mut self,
        status: ServiceStatus,
        timeout_duration: Option<Duration>,
    ) -> Result<ServiceStatus, ServiceStatus> {
        let current = *self.0.borrow();
        if status == current {
            return Ok(current);
        }
        let timeout_duration = timeout_duration.unwrap_or_else(|| Duration::from_secs(u64::MAX));
        tokio::time::timeout(timeout_duration, self.0.wait_for(|s| s == &status))
            .await
            .map(|r| r.map(|s| *s).map_err(|_| current))
            .unwrap_or(Err(current))
    }
}

pub struct StatusHandle {
    updater: Arc<StatusUpdater>,
    watcher: StatusWatcher,
}

impl Clone for StatusHandle {
    fn clone(&self) -> Self {
        Self {
            updater: Arc::clone(&self.updater),
            watcher: self.watcher.clone(),
        }
    }
}

impl StatusHandle {
    #[must_use]
    pub fn new() -> Self {
        let (updater, watcher) = watch::channel(ServiceStatus::Uninitialized);
        let updater = Arc::new(StatusUpdater(updater));
        let watcher = StatusWatcher(watcher);
        Self { updater, watcher }
    }

    #[expect(
        clippy::missing_const_for_fn,
        reason = "We dereference an `Arc`, which is not const"
    )]
    #[must_use]
    pub fn updater(&self) -> &StatusUpdater {
        &self.updater
    }

    #[must_use]
    pub fn watcher(&self) -> StatusWatcher {
        self.watcher.clone()
    }

    #[must_use]
    pub fn borrow(&self) -> Ref<'_, ServiceStatus> {
        self.watcher.0.borrow()
    }
}

impl Default for StatusHandle {
    fn default() -> Self {
        Self::new()
    }
}
