use crate::services::{
    life_cycle::LifecycleHandle,
    state::{StateHandle, StateOperator},
    state_handle::ServiceStateHandle,
    AsServiceId, ServiceCore,
};

/// Executor for a `Service`.
///
/// Contains all the necessary information to run a `Service`.
pub struct ServiceRunner<Message, Settings, State, StateOperator, RuntimeServiceId> {
    service_state: ServiceStateHandle<Message, Settings, State, RuntimeServiceId>,
    state_handle: StateHandle<State, StateOperator>,
    lifecycle_handle: LifecycleHandle,
    initial_state: State,
}

impl<Message, Settings, State, StateOp, RuntimeServiceId>
    ServiceRunner<Message, Settings, State, StateOp, RuntimeServiceId>
{
    pub const fn new(
        service_state: ServiceStateHandle<Message, Settings, State, RuntimeServiceId>,
        state_handle: StateHandle<State, StateOp>,
        lifecycle_handle: LifecycleHandle,
        initial_state: State,
    ) -> Self {
        Self {
            service_state,
            state_handle,
            lifecycle_handle,
            initial_state,
        }
    }
}

impl<Message, Settings, State, StateOp, RuntimeServiceId>
    ServiceRunner<Message, Settings, State, StateOp, RuntimeServiceId>
where
    State: Clone + Send + Sync + 'static,
    StateOp: StateOperator<State = State> + Send + 'static,
{
    /// Spawn the service main loop and handle its lifecycle.
    ///
    /// # Returns
    ///
    /// A tuple containing the service id and the lifecycle handle, which allows
    /// to manually abort the execution.
    ///
    /// # Errors
    ///
    /// If the service cannot be initialized properly with the retrieved state.
    pub fn run<Service>(self) -> Result<LifecycleHandle, crate::DynError>
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        RuntimeServiceId: AsServiceId<Service>,
    {
        let Self {
            service_state,
            state_handle,
            lifecycle_handle,
            initial_state,
            ..
        } = self;

        let runtime = service_state.overwatch_handle.runtime().clone();
        let service = Service::init(service_state, initial_state)?;

        runtime.spawn(service.run());
        runtime.spawn(state_handle.run());

        Ok(lifecycle_handle)
    }
}
