use crate::services::status::{
    channel, service_status::ServiceStatus, updater::StatusUpdater, watcher::StatusWatcher,
};

// Define the service for which the StatusUpdater is being created
pub struct ServiceRunnerAPI;
pub struct ServiceAPI;

/// The manager for the status of the service. Consists of a sender and a
/// receiver.
///
/// The intended purpose is to let `Service`s communicate when they've finished
/// initialising and are ready to operate.
pub struct StatusHandle {
    service_runner_updater: StatusUpdater<ServiceRunnerAPI>,
    service_updater: StatusUpdater<ServiceAPI>,
    watcher: StatusWatcher,
}

impl StatusHandle {
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = channel(ServiceStatus::Stopped);
        let service_runner_updater = StatusUpdater::<ServiceRunnerAPI>::new(sender.clone());
        let service_updater = StatusUpdater::<ServiceAPI>::new(sender);
        let watcher = StatusWatcher::new(receiver);
        Self {
            service_runner_updater,
            service_updater,
            watcher,
        }
    }

    #[must_use]
    pub const fn service_runner_updater(&self) -> &StatusUpdater<ServiceRunnerAPI> {
        &self.service_runner_updater
    }

    #[must_use]
    pub const fn service_updater(&self) -> &StatusUpdater<ServiceAPI> {
        &self.service_updater
    }

    #[must_use]
    pub const fn watcher(&self) -> &StatusWatcher {
        &self.watcher
    }
}

impl Default for StatusHandle {
    fn default() -> Self {
        Self::new()
    }
}
