use tokio::task::JoinHandle;

use crate::services::service_handle::ServiceHandle;

pub struct ServiceRunnerHandle<Message, Settings, State, StateOperator, RuntimeServiceId> {
    service_handle: ServiceHandle<Message, Settings, State, StateOperator, RuntimeServiceId>,
    runner_join_handle: JoinHandle<()>,
}

impl<Message, Settings, State, StateOperator, RuntimeServiceId>
    ServiceRunnerHandle<Message, Settings, State, StateOperator, RuntimeServiceId>
{
    pub const fn new(
        service_handle: ServiceHandle<Message, Settings, State, StateOperator, RuntimeServiceId>,
        runner_join_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            service_handle,
            runner_join_handle,
        }
    }

    pub const fn service_handle(
        &self,
    ) -> &ServiceHandle<Message, Settings, State, StateOperator, RuntimeServiceId> {
        &self.service_handle
    }

    pub const fn runner_join_handle(&self) -> &JoinHandle<()> {
        &self.runner_join_handle
    }

    pub fn runner_join_handle_owned(self) -> JoinHandle<()> {
        self.runner_join_handle
    }
}
