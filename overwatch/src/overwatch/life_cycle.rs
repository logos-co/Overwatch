// std
use std::borrow::Cow;
use std::collections::HashMap;
use std::default::Default;
use std::error::Error;
// crates
use tokio::sync::broadcast::Sender;
// internal
use crate::services::life_cycle::{FinishedSignal, LifecycleHandle, LifecycleMessage};
use crate::services::ServiceId;
use crate::DynError;

/// Grouper handle for the [`LifecycleHandle`] of each spawned service.
#[derive(Clone)]
pub struct ServicesLifeCycleHandle {
    handlers: HashMap<ServiceId, LifecycleHandle>,
}

impl ServicesLifeCycleHandle {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            handlers: HashMap::default(),
        }
    }

    /// Send a `Shutdown` message to the specified service.
    ///
    /// # Arguments
    ///
    /// `service` - The [`ServiceId`] of the target service
    /// `sender` - The sender side of a broadcast channel. It's expected that once the receiver
    /// finishes processing the message, a signal will be sent back.
    ///
    /// # Errors
    ///
    /// The error returned when trying to send the shutdown command to the specified service.
    ///
    /// # Panics
    /// If the specified service handler is not available.
    pub fn shutdown(
        &self,
        service: ServiceId,
        sender: Sender<FinishedSignal>,
    ) -> Result<(), DynError> {
        self.handlers
            .get(service)
            .unwrap()
            .send(LifecycleMessage::Shutdown(sender))?;
        Ok(())
    }

    /// Send a [`LifecycleMessage::Kill`] message to the specified service ([`ServiceId`])
    ///[`crate::overwatch::OverwatchRunner`].
    /// # Arguments
    ///
    /// `service` - The [`ServiceId`] of the target service
    ///
    /// # Errors
    ///
    /// The error returned when trying to send the kill command to the specified service.
    ///
    /// # Panics
    /// If the specified service handler is not available.
    pub fn kill(&self, service: ServiceId) -> Result<(), DynError> {
        self.handlers
            .get(service)
            .unwrap()
            .send(LifecycleMessage::Kill)
    }

    /// Send a [`LifecycleMessage::Kill`] message to all services registered in this handle.
    ///
    /// # Errors
    ///
    /// The error returned when trying to send the kill command to any of the running services.
    pub fn kill_all(&self) -> Result<(), DynError> {
        for service_id in self.services_ids() {
            self.kill(service_id)?;
        }
        Ok(())
    }

    /// Get all [`ServiceId`]s registered in this handle
    pub fn services_ids(&self) -> impl Iterator<Item = ServiceId> + '_ {
        self.handlers.keys().copied()
    }
}

impl<const N: usize> TryFrom<[(ServiceId, LifecycleHandle); N]> for ServicesLifeCycleHandle {
    // TODO: On errors refactor extract into a concrete error type with `thiserror`
    type Error = Box<dyn Error + Send + Sync>;

    fn try_from(value: [(ServiceId, LifecycleHandle); N]) -> Result<Self, Self::Error> {
        let mut handlers = HashMap::new();
        for (service_id, handle) in value {
            if handlers.contains_key(service_id) {
                return Err(Box::<dyn Error + Send + Sync>::from(Cow::Owned(format!(
                    "Duplicated serviceId: {service_id}"
                ))));
            }
            handlers.insert(service_id, handle);
        }
        Ok(Self { handlers })
    }
}
