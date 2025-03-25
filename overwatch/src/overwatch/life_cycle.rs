use tokio::sync::broadcast::Sender;

use crate::services::life_cycle::FinishedSignal;

/// A trait for handling the lifecycle [`LifecycleHandle`] of spawned services.
pub trait ServicesLifeCycleHandle<RuntimeServiceId> {
    /// The error for different operations.
    type Error;

    /// Shut down a service.
    ///
    /// # Errors
    ///
    /// If the shutdown fails.
    fn shutdown(
        &self,
        service: &RuntimeServiceId,
        sender: Sender<FinishedSignal>,
    ) -> Result<(), Self::Error>;

    /// Kill a service.
    ///
    /// # Errors
    ///
    /// If the kill fails.
    fn kill(&self, service: &RuntimeServiceId) -> Result<(), Self::Error>;

    /// Kill all services.
    ///
    /// # Errors
    ///
    /// If the kill fails.
    fn kill_all(&self) -> Result<(), Self::Error>;
}
