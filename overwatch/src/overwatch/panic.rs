use std::{
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
    pub(crate) fn new(service_id: RuntimeServiceId, message: impl Into<String>) -> Self {
        Self {
            service_id,
            message: message.into(),
        }
    }
}

impl<RuntimeServiceId: Display> Display for ServicePanic<RuntimeServiceId> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Service {} panicked: {}", self.service_id, self.message)
    }
}

impl<RuntimeServiceId: Debug + Display> Error for ServicePanic<RuntimeServiceId> {}

/// How [`Overwatch`](crate::overwatch::Overwatch) finished executing: `Ok`
/// after a regular shutdown, or the [`ServicePanic`] that caused the shutdown.
pub type ExitResult<RuntimeServiceId> = Result<(), ServicePanic<RuntimeServiceId>>;

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
pub struct Shutdown;

#[async_trait]
impl<RuntimeServiceId> PanicPolicy<RuntimeServiceId> for Shutdown
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

/// An optional policy.
///
/// `Some` behaves as the policy it holds. `None` does nothing besides logging
/// the panic: only the `Service` that panicked is stopped, the rest keep
/// running.
///
/// The default is `None`, which is what
/// [`OverwatchRunner::run`](crate::overwatch::OverwatchRunner::run) uses.
#[async_trait]
impl<RuntimeServiceId, Policy> PanicPolicy<RuntimeServiceId> for Option<Policy>
where
    RuntimeServiceId: Display + Send + Sync + 'static,
    Policy: PanicPolicy<RuntimeServiceId>,
{
    async fn on_service_panic(
        &self,
        service_panic: ServicePanic<RuntimeServiceId>,
        overwatch_handle: &OverwatchHandle<RuntimeServiceId>,
    ) {
        match self {
            Some(panic_policy) => {
                panic_policy
                    .on_service_panic(service_panic, overwatch_handle)
                    .await;
            }
            None => {
                warn!(
                    "No panic policy, Overwatch keeps running after a service panic: {service_panic}"
                );
            }
        }
    }
}
