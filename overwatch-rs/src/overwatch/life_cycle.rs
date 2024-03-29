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

/// Grouper handle for the `LifecycleHandle` of each spawned service.
#[derive(Clone)]
pub struct ServicesLifeCycleHandle {
    handlers: HashMap<ServiceId, LifecycleHandle>,
}

impl ServicesLifeCycleHandle {
    pub fn empty() -> Self {
        Self {
            handlers: Default::default(),
        }
    }

    /// Send a `Shutdown` message to the specified service
    ///
    /// # Arguments
    ///
    /// `service` - The `ServiceId` of the target service
    /// `sender` - A sender side of a broadcast channel. A return signal when finished handling the
    /// message will be sent.
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

    /// Send a `Kill` message to the specified service (`ServiceId`)
    ///
    /// # Arguments
    ///
    /// `service` - The `ServiceId` of the target service
    pub fn kill(&self, service: ServiceId) -> Result<(), DynError> {
        self.handlers
            .get(service)
            .unwrap()
            .send(LifecycleMessage::Kill)
    }

    /// Send a `Kill` message to all services registered in this handle
    pub fn kill_all(&self) -> Result<(), DynError> {
        for service_id in self.services_ids() {
            self.kill(service_id)?;
        }
        Ok(())
    }

    /// Get all services ids registered in this handle
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
