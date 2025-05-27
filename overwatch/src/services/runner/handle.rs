use tokio::task::JoinHandle;

use crate::services::service_handle::ServiceHandle;

pub struct ServiceRunnerHandle<Message, Settings, State, StateOperator> {
    service_handle: ServiceHandle<Message, Settings, State, StateOperator>,
    runner_join_handle: JoinHandle<()>,
}

impl<Message, Settings, State, StateOperator>
    ServiceRunnerHandle<Message, Settings, State, StateOperator>
{
    pub const fn new(
        service_handle: ServiceHandle<Message, Settings, State, StateOperator>,
        runner_join_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            service_handle,
            runner_join_handle,
        }
    }

    pub const fn service_handle(&self) -> &ServiceHandle<Message, Settings, State, StateOperator> {
        &self.service_handle
    }

    pub const fn runner_join_handle(&self) -> &JoinHandle<()> {
        &self.runner_join_handle
    }

    pub fn runner_join_handle_owned(self) -> JoinHandle<()> {
        self.runner_join_handle
    }
}
