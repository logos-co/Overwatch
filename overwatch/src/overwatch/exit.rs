use std::{
    any::Any,
    fmt::{Display, Formatter},
};

/// What [`Overwatch`](crate::overwatch::Overwatch) does when a `Service`'s
/// [`run`](crate::services::ServiceCore::run) panics.
///
/// In both cases the panic is caught, the `Service` is cleaned up and its
/// status becomes
/// [`ServiceStatus::Failed`](crate::services::status::ServiceStatus::Failed).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ServicePanicPolicy {
    /// Shut [`Overwatch`](crate::overwatch::Overwatch) down: every `Service`
    /// is stopped and
    /// [`Overwatch::wait_finished`](crate::overwatch::Overwatch::wait_finished)
    /// returns [`OverwatchExit::ServicePanicked`].
    #[default]
    ShutdownOverwatch,
    /// Stop only the `Service` that panicked. The rest keep running.
    StopService,
}

/// A panic caught in a `Service`'s [`run`](crate::services::ServiceCore::run).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServicePanic<RuntimeServiceId> {
    /// The `Service` that panicked.
    pub service_id: RuntimeServiceId,
    /// The panic message.
    pub message: String,
}

impl<RuntimeServiceId> ServicePanic<RuntimeServiceId> {
    pub(crate) fn new(service_id: RuntimeServiceId, payload: &(dyn Any + Send)) -> Self {
        let message = payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "Non-string panic payload".to_owned());
        Self {
            service_id,
            message,
        }
    }
}

impl<RuntimeServiceId: Display> Display for ServicePanic<RuntimeServiceId> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Service {} panicked: {}", self.service_id, self.message)
    }
}

/// Why [`Overwatch`](crate::overwatch::Overwatch) finished executing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverwatchExit<RuntimeServiceId> {
    /// A shutdown was requested through
    /// [`OverwatchHandle::shutdown`](crate::overwatch::OverwatchHandle::shutdown).
    Shutdown,
    /// A `Service` panicked and the [`ServicePanicPolicy`] was
    /// [`ServicePanicPolicy::ShutdownOverwatch`].
    ServicePanicked(ServicePanic<RuntimeServiceId>),
}
