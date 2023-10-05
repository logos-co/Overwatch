use crate::services::life_cycle::{FinishedSignal, LifecycleHandle, LifecycleMessage};
use crate::services::ServiceId;
use crate::DynError;
use std::collections::HashMap;
use tokio::sync::broadcast::Sender;

#[derive(Clone)]
pub struct ServicesLifeCycleHandle {
    handlers: HashMap<ServiceId, LifecycleHandle>,
}

impl ServicesLifeCycleHandle {
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

    pub fn kill(&self, service: ServiceId) -> Result<(), DynError> {
        self.handlers
            .get(service)
            .unwrap()
            .send(LifecycleMessage::Kill)
    }
    pub fn kill_all(&self) -> Result<(), DynError> {
        for service_id in self.services_ids() {
            self.kill(service_id)?;
        }
        Ok(())
    }

    pub fn services_ids(&self) -> impl Iterator<Item = ServiceId> + '_ {
        self.handlers.keys().copied()
    }
}

impl<I: IntoIterator<Item = (ServiceId, LifecycleHandle)>> From<I> for ServicesLifeCycleHandle {
    fn from(value: I) -> Self {
        Self {
            handlers: value.into_iter().collect(),
        }
    }
}
