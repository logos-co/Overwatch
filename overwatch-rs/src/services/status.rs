// std
use std::default::Default;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
// crates
use crate::services::{ServiceData, ServiceId};
use thiserror::Error;
use tokio::sync::watch;
// internal

#[derive(Error, Debug)]
pub enum ServiceStatusError {
    #[error("service {service_id} is not available")]
    Unavailable { service_id: ServiceId },
}

pub type ServiceStatusResult = Result<StatusWatcher, ServiceStatusError>;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ServiceStatus {
    Uninitialized,
    Running,
    Stopped,
}

pub struct StatusUpdater(watch::Sender<ServiceStatus>);

impl StatusUpdater {
    pub fn update(&self, status: ServiceStatus) {
        self.0
            .send(status)
            .expect("Overwatch always maintain an open watcher, send should always succeed");
    }
}

#[derive(Debug, Clone)]
pub struct StatusWatcher(watch::Receiver<ServiceStatus>);

impl StatusWatcher {
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

pub struct StatusHandle<Status> {
    updater: Arc<StatusUpdater>,
    watcher: StatusWatcher,
    _phantom: PhantomData<Status>,
}

impl<Status> Clone for StatusHandle<Status>
where
    Status: ServiceData,
{
    fn clone(&self) -> Self {
        Self {
            updater: Arc::clone(&self.updater),
            watcher: self.watcher.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<Status> StatusHandle<Status> {
    #[must_use]
    pub fn new() -> Self {
        let (updater, watcher) = watch::channel(ServiceStatus::Uninitialized);
        let updater = Arc::new(StatusUpdater(updater));
        let watcher = StatusWatcher(watcher);
        Self {
            updater,
            watcher,
            _phantom: PhantomData,
        }
    }
    #[must_use]
    pub fn updater(&self) -> &StatusUpdater {
        &self.updater
    }
    #[must_use]
    pub fn watcher(&self) -> StatusWatcher {
        self.watcher.clone()
    }
}

impl<Status> Default for StatusHandle<Status> {
    fn default() -> Self {
        Self::new()
    }
}
