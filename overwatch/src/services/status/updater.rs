use std::marker::PhantomData;

use crate::services::status::{
    Sender,
    handle::{ServiceAPI, ServiceRunnerAPI},
    service_status::ServiceStatus,
};

/// Sender of [`ServiceStatus`] updates.
///
/// Underneath it's a state machine, where each [`ServiceStatus`] only
/// transitions to the next state.
///
/// The usage of the [`StatusUpdater`] is divided between `Service` and the
/// [`ServiceRunner`](crate::services::runner::ServiceRunner):
/// - `Service` uses the [`StatusUpdater`] to signal when it is ready to
///   operate.
/// - [`ServiceRunnerAPI`] uses the [`StatusUpdater`] to signal when the
///   `Service` is either in the process of starting or has been stopped.
///
/// # Note
///
/// The current implementation takes advantage of the `Clone` capability of the
/// `sender` to give the `Service` a single `StatusUpdater<Ready>` instance that
/// can only be used to set the status to `Ready`.
pub struct StatusUpdater<API> {
    sender: Sender,
    _api: PhantomData<API>,
}

// When using `#[derive(Clone)]`, calling `.clone()` returns a reference not a
// value.
impl<API> Clone for StatusUpdater<API> {
    fn clone(&self) -> Self {
        Self::new(self.sender.clone())
    }
}

impl<API> StatusUpdater<API> {
    #[must_use]
    pub const fn new(sender: Sender) -> Self {
        Self {
            sender,
            _api: PhantomData,
        }
    }

    /// Send a [`ServiceStatus`] update message to the associated service.
    ///
    /// # Panics
    ///
    /// If the message cannot be sent to the target service.
    fn send(&self, status: ServiceStatus) {
        self.sender
            .send(status)
            .expect("Overwatch always maintains an open watcher, send should always succeed");
    }
}

/// [`StatusUpdater`] implementation for the
/// [`ServiceRunner`](crate::services::runner::ServiceRunner).
impl StatusUpdater<ServiceRunnerAPI> {
    /// Shorthand for sending a [`ServiceStatus::Starting`] message.
    pub fn notify_starting(&self) {
        self.send(ServiceStatus::Starting);
    }

    /// Shorthand for sending a [`ServiceStatus::Stopped`] message.
    pub fn notify_stopped(&self) {
        self.send(ServiceStatus::Stopped);
    }
}

/// [`StatusUpdater`] implementation for the `Service`.
impl StatusUpdater<ServiceAPI> {
    /// Shorthand for sending a [`ServiceStatus::Ready`] message.
    ///
    /// # Notes
    ///
    /// - Calling this method multiple times has no additional effect.
    /// - Ideally, because this is used by the `Service`, it would take
    ///   ownership of the `StatusUpdater` so that it can only be used once.
    ///   However, this triggers partial move issues in a lot of scenarios.
    pub fn notify_ready(&self) {
        self.send(ServiceStatus::Ready);
    }
}
