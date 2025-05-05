use std::fmt::{Debug, Display};

use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::info;

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        handle::ServiceHandle,
        life_cycle::{LifecycleHandle, LifecycleMessage},
        relay::{InboundRelay, Relay},
        resources::ServiceResources,
        settings::SettingsUpdater,
        state::{ServiceState, StateHandle, StateOperator},
        status::StatusHandle,
        AsServiceId, ServiceCore,
    },
    DynError,
};

/// Executor for a `Service`.
///
/// Contains all the necessary information to run a `Service`.
pub struct ServiceRunner<Message, Settings, State, StateOperator, RuntimeServiceId> {
    service_resources: ServiceResources<Message, Settings, State, RuntimeServiceId>,
    state_handle: StateHandle<State, StateOperator>,
    lifecycle_handle: LifecycleHandle,
    overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    settings_updater: SettingsUpdater<Settings>,
    status_handle: StatusHandle,
    relay: Relay<Message>,
    relay_buffer_size: usize,
}

impl<Message, Settings, State, StateOp, RuntimeServiceId>
    ServiceRunner<Message, Settings, State, StateOp, RuntimeServiceId>
where
    Settings: Clone,
    State: ServiceState<Settings = Settings> + Clone,
    <State as ServiceState>::Error: Display + Debug,
    StateOp: StateOperator<State = State> + Clone,
    RuntimeServiceId: Clone,
{
    /// Creates a new `ServiceRunner`.
    ///
    /// # Panics
    ///
    /// If the state cannot be created from the settings.
    #[must_use]
    pub fn new(
        settings: Settings,
        overwatch_handle: OverwatchHandle<RuntimeServiceId>,
        relay_buffer_size: usize,
    ) -> Self {
        let lifecycle_handle = LifecycleHandle::new();
        let relay = Relay::new(relay_buffer_size);
        let status_handle = StatusHandle::new();
        let state_operator = StateOp::from_settings(&settings);
        let settings_updater = SettingsUpdater::new(settings);

        let (state_handle, state_updater) = StateHandle::<State, StateOp>::new(state_operator);

        let service_resources = ServiceResources::new(
            status_handle.clone(),
            overwatch_handle.clone(),
            settings_updater.clone(),
            state_updater,
            lifecycle_handle.clone(),
        );

        Self {
            service_resources,
            state_handle,
            lifecycle_handle,
            overwatch_handle,
            settings_updater,
            status_handle,
            relay,
            relay_buffer_size,
        }
    }
}

impl<Message, Settings, State, StateOp, RuntimeServiceId>
    ServiceRunner<Message, Settings, State, StateOp, RuntimeServiceId>
where
    State: Clone + Send + Sync + 'static,
    StateOp: StateOperator<State = State> + Send + 'static,
    // New stuff
    State: ServiceState<Settings = Settings>,
    Message: 'static + Send + Sync,
    Settings: Clone + 'static + Sync + Send,
    RuntimeServiceId: 'static + Clone + Send,
    <State as ServiceState>::Error: Display + Debug,
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
    pub fn run<Service>(self) -> ServiceHandle<Message, Settings, State, StateOp, RuntimeServiceId>
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        RuntimeServiceId: AsServiceId<Service>,
        StateOp: Clone, // For StateHandle::clone
    {
        let service_handle = ServiceHandle::from(&self);
        let runtime = self.service_resources.overwatch_handle.runtime().clone();
        let _lifecycle_task_handle = runtime.spawn(self.run_::<Service>());

        service_handle
    }

    async fn run_<Service>(self)
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,          // For StateHandle::clone
        RuntimeServiceId: Clone, // For ServiceStateHandle::into
    {
        let Self {
            service_resources,
            state_handle,
            lifecycle_handle,
            relay,
            relay_buffer_size,
            ..
        } = self;

        let Relay {
            inbound,
            outbound: _,
            consumer_sender,
            consumer_receiver,
        } = relay;

        let runtime = service_resources.overwatch_handle.runtime().clone();
        let mut lifecycle_stream = lifecycle_handle.message_stream();
        let mut service_task_handle: Option<_> = None;
        let mut state_handle_task_handle: Option<_> = None;

        let mut inbound_relay = Some(inbound);

        while let Some(lifecycle_message) = lifecycle_stream.next().await {
            match lifecycle_message {
                LifecycleMessage::Start(sender) => {
                    let initial_state_result = Self::get_service_initial_state(&service_resources);
                    let Ok(initial_state) = initial_state_result else {
                        // TODO: Print error message
                        panic!("Failed to create initial state from settings");
                    };

                    let inbound_relay = inbound_relay.take().expect("Inbound relay must exist");
                    let services_resources_handle = service_resources.to_handle(inbound_relay);

                    // TODO: Better to auto-handle inside the StateOperator
                    service_resources
                        .state_updater
                        .update(initial_state.clone());

                    let service = Service::init(services_resources_handle, initial_state);

                    match service {
                        Ok(service) => {
                            let state_handle = state_handle.clone();
                            service_task_handle = Some(runtime.spawn(service.run()));
                            state_handle_task_handle = Some(runtime.spawn(state_handle.run()));
                            sender
                                .send(())
                                .expect("Failed sending the Start FinishedSignal.");
                        }
                        Err(error) => {
                            panic!("Error while initialising service: {error}");
                        }
                    }
                }
                LifecycleMessage::Shutdown(sender) => {
                    Self::stop_service(&mut service_task_handle, &mut state_handle_task_handle);
                    let consumer = consumer_receiver
                        .recv()
                        .expect("Consumer must be retrieved.");
                    inbound_relay = Some(InboundRelay::new(
                        consumer,
                        consumer_sender.clone(),
                        relay_buffer_size,
                    ));
                    sender
                        .send(())
                        .expect("Failed sending the Shutdown FinishedSignal.");
                }
                LifecycleMessage::Kill => {
                    // TODO: Remove branch
                    Self::stop_service(&mut service_task_handle, &mut state_handle_task_handle);
                }
            }
        }
    }

    /// Retrieves the initial state for the service.
    ///
    /// First tries to load the state from the operator (a previously saved
    /// state). If it fails, it defaults to the initial state created from
    /// the settings.
    fn get_service_initial_state(
        service_resources: &ServiceResources<Message, Settings, State, RuntimeServiceId>,
    ) -> Result<State, State::Error> {
        let settings = service_resources
            .settings_updater
            .notifier()
            .get_updated_settings();
        if let Ok(Some(loaded_state)) = StateOp::try_load(&settings) {
            info!("Loaded state from Operator");
            Ok(loaded_state)
        } else {
            info!("Couldn't load state from Operator. Creating from settings.");
            State::from_settings(&settings)
        }
    }

    fn stop_service(
        service_task_handle: &mut Option<JoinHandle<Result<(), DynError>>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
    ) {
        if let Some(handle) = service_task_handle.take() {
            handle.abort_handle().abort();
        }
        if let Some(handle) = state_handle_task_handle.take() {
            handle.abort_handle().abort();
        }
    }
}

impl<Message, Settings, State, Operator, RuntimeServiceId>
    From<&ServiceRunner<Message, Settings, State, Operator, RuntimeServiceId>>
    for ServiceHandle<Message, Settings, State, Operator, RuntimeServiceId>
where
    Settings: Clone,
    State: Clone,
    Operator: Clone,
    RuntimeServiceId: Clone,
{
    fn from(
        service_runner: &ServiceRunner<Message, Settings, State, Operator, RuntimeServiceId>,
    ) -> Self {
        Self::new(
            service_runner.relay.outbound.clone(),
            service_runner.overwatch_handle.clone(),
            service_runner.settings_updater.clone(),
            service_runner.status_handle.clone(),
            service_runner.state_handle.clone(),
            service_runner.lifecycle_handle.clone(),
        )
    }
}
