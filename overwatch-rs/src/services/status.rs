// std
use std::marker::PhantomData;
use std::time::Duration;
// crates
use crate::services::ServiceData;
use tokio::sync::watch;
// internal

#[derive(Copy, Clone, Eq, PartialEq)]
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
            .expect("Overwatch always maintain an open watcher, send should always succeed")
    }
}

#[derive(Clone)]
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

pub struct StatusHandle<S: ServiceData> {
    updater: StatusUpdater,
    watcher: StatusWatcher,
    _phantom: PhantomData<S>,
}

impl<S: ServiceData> StatusHandle<S> {
    pub fn new() -> Self {
        let (updater, watcher) = watch::channel(ServiceStatus::Uninitialized);
        let updater = StatusUpdater(updater);
        let watcher = StatusWatcher(watcher);
        Self {
            updater,
            watcher,
            _phantom: Default::default(),
        }
    }
    pub fn updater(&self) -> &StatusUpdater {
        &self.updater
    }

    pub fn watcher(&self) -> StatusWatcher {
        self.watcher.clone()
    }
}

impl<S: ServiceData> Default for StatusHandle<S> {
    fn default() -> Self {
        Self::new()
    }
}
