use std::fmt::Display;

use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::info;

use crate::services::handle::ServiceHandle;
use crate::{
    services::{
        life_cycle::{LifecycleHandle, LifecycleMessage},
        relay::{ConsumerReceiver, ConsumerSender, InboundRelay},
        state::{ServiceState, StateHandle, StateOperator},
        state_handle::ServiceResources,
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
    inbound_relay: InboundRelay<Message>,
    consumer_sender: ConsumerSender<Message>,
    consumer_receiver: ConsumerReceiver<Message>,
    relay_buffer_size: usize,
}

impl<Message, Settings, State, StateOp, RuntimeServiceId>
    ServiceRunner<Message, Settings, State, StateOp, RuntimeServiceId>
{
    #![expect(
        clippy::too_many_arguments,
        reason = "Refactor in progress. This will be simplified."
    )]
    #[must_use]
    pub const fn new(
        service_resources: ServiceResources<Message, Settings, State, RuntimeServiceId>,
        state_handle: StateHandle<State, StateOp>,
        lifecycle_handle: LifecycleHandle,
        inbound_relay: InboundRelay<Message>,
        consumer_sender: ConsumerSender<Message>,
        consumer_receiver: ConsumerReceiver<Message>,
        relay_buffer_size: usize,
    ) -> Self {
        Self {
            service_resources,
            state_handle,
            lifecycle_handle,
            inbound_relay,
            consumer_sender,
            consumer_receiver,
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
    <State as ServiceState>::Error: Display + std::fmt::Debug,
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
        // Return a copy of the lifecycle handle to the caller, it's the same that will
        // be passed to the service.
        let lifecycle_handle = self.lifecycle_handle.clone();
        let overwatch_handle = self.service_resources.overwatch_handle.clone();

        let service_handle = ServiceHandle::new(
            settings,
            overwatch_handle,
            lifecycle_handle,
            self.relay_buffer_size,
        )?;

        let runtime = self.service_resources.overwatch_handle.runtime();
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
            inbound_relay,
            consumer_sender,
            consumer_receiver,
            service_resources,
            state_handle,
            lifecycle_handle,
            relay_buffer_size,
        } = self;

        let mut service_task_handle: Option<_> = None;
        let mut state_handle_task_handle: Option<_> = None;
        let mut inbound_relay = Some(inbound_relay);

        let runtime = service_resources.overwatch_handle.runtime().clone();
        let mut lifecycle_stream = lifecycle_handle.message_stream();

        while let Some(lifecycle_message) = lifecycle_stream.next().await {
            match lifecycle_message {
                LifecycleMessage::Start(sender) => {
                    // TODO: Better error handling
                    let initial_state = Self::get_service_initial_state(&service_resources)
                        .expect("Couldn't build initial state");

                    let inbound_relay = inbound_relay.take().expect("Inbound relay must exist");
                    let services_resources_handle = service_resources.to_handle(inbound_relay);
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

    fn get_service_initial_state(
        service_resources: &ServiceResources<Message, Settings, State, RuntimeServiceId>,
    ) -> Result<State, <State as ServiceState>::Error> {
        let settings = service_resources.settings_reader.get_updated_settings();
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
