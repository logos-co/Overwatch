use tokio::sync::watch;

pub mod handle;
pub mod service_status;
pub mod updater;
pub mod watcher;

pub use handle::StatusHandle;
pub use service_status::ServiceStatus;
pub use updater::StatusUpdater;
pub use watcher::StatusWatcher;

pub(crate) type Sender = watch::Sender<ServiceStatus>;
pub(crate) type Receiver = watch::Receiver<ServiceStatus>;
pub(crate) fn channel(init: ServiceStatus) -> (Sender, Receiver) {
    watch::channel(init)
}
