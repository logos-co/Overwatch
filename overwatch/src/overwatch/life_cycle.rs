use tokio::sync::broadcast::Sender;

use crate::services::life_cycle::FinishedSignal;

pub trait ServicesLifeCycleHandle<RuntimeServiceId> {
    type Error;

    fn shutdown(
        &self,
        service: &RuntimeServiceId,
        sender: Sender<FinishedSignal>,
    ) -> Result<(), Self::Error>;

    fn kill(&self, service: &RuntimeServiceId) -> Result<(), Self::Error>;

    fn kill_all(&self) -> Result<(), Self::Error>;
}
