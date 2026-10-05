use std::{
    any::Any,
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use async_trait::async_trait;
use tracing::{error, warn};

use crate::overwatch::OverwatchHandle;

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

impl<RuntimeServiceId: Debug + Display> Error for ServicePanic<RuntimeServiceId> {}

/// What to do when a `Service`'s [`run`](crate::services::ServiceCore::run)
/// panics.
///
/// There is a single policy for all the `Service`s of an
/// [`Overwatch`](crate::overwatch::Overwatch). Its type is
/// [`Services::PanicPolicy`](crate::overwatch::Services::PanicPolicy) and its
/// instance is either the type's default or the one given to
/// [`OverwatchRunner::run_with_panic_policy`](crate::overwatch::OverwatchRunner::run_with_panic_policy).
///
/// By the time the policy is called, the `Service` has already been cleaned
/// up and its status is
/// [`ServiceStatus::Failed`](crate::services::status::ServiceStatus::Failed).
/// The policy runs in its own task, so it's free to use the
/// [`OverwatchHandle`], including to stop or restart the `Service` that
/// panicked.
#[async_trait]
pub trait PanicPolicy<RuntimeServiceId>: Send + Sync + 'static {
    /// Called once for every panic caught in a `Service`.
    async fn on_service_panic(
        &self,
        service_panic: ServicePanic<RuntimeServiceId>,
        overwatch_handle: &OverwatchHandle<RuntimeServiceId>,
    );
}

/// Shut [`Overwatch`](crate::overwatch::Overwatch) down: every `Service` is
/// stopped and
/// [`Overwatch::wait_finished`](crate::overwatch::Overwatch::wait_finished)
/// returns the [`ServicePanic`] as an error.
#[derive(Copy, Clone, Debug, Default)]
pub struct ShutdownOverwatch;

#[async_trait]
impl<RuntimeServiceId> PanicPolicy<RuntimeServiceId> for ShutdownOverwatch
where
    RuntimeServiceId: Debug + Display + Send + Sync + 'static,
{
    async fn on_service_panic(
        &self,
        service_panic: ServicePanic<RuntimeServiceId>,
        overwatch_handle: &OverwatchHandle<RuntimeServiceId>,
    ) {
        if let Err(error) = overwatch_handle.shutdown_with_panic(service_panic).await {
            error!("Error while shutting down Overwatch after a service panic: {error}");
        }
    }
}

/// Do nothing besides logging the panic: only the `Service` that panicked is
/// stopped, the rest keep running.
#[derive(Copy, Clone, Debug, Default)]
pub struct NoPolicy;

#[async_trait]
impl<RuntimeServiceId> PanicPolicy<RuntimeServiceId> for NoPolicy
where
    RuntimeServiceId: Display + Send + Sync + 'static,
{
    async fn on_service_panic(
        &self,
        service_panic: ServicePanic<RuntimeServiceId>,
        _overwatch_handle: &OverwatchHandle<RuntimeServiceId>,
    ) {
        warn!("No panic policy, Overwatch keeps running after a service panic: {service_panic}");
    }
}
