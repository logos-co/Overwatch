use tokio::sync::watch;

mod all;
pub mod handle;
pub mod service_status;
pub mod updater;
pub mod watcher;

pub use all::{
    CompletionPolicy, DependencyObservation, WaitForAllError, WaitForAllErrorKind,
    wait_for_all_with_policy,
};
pub use handle::StatusHandle;
pub use service_status::ServiceStatus;
pub use updater::StatusUpdater;
pub use watcher::{StatusWatcher, WaitForError, WaitPolicy};

pub(crate) type Sender = watch::Sender<ServiceStatus>;
pub(crate) type Receiver = watch::Receiver<ServiceStatus>;
pub(crate) fn channel(init: ServiceStatus) -> (Sender, Receiver) {
    watch::channel(init)
}
