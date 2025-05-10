use std::fmt::Display;

use tokio::{runtime::Handle, sync::broadcast::Sender, task::JoinHandle};
use tokio_stream::StreamExt;
use tracing::info;

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        handle::ServiceHandle,
        life_cycle::{FinishedSignal, LifecycleHandle, LifecycleMessage},
        relay::{ConsumerReceiver, ConsumerSender, InboundRelay, Relay},
        resources::ServiceResources,
        settings::SettingsUpdater,
        state::{ServiceState, StateHandle, StateOperator},
        status::{ServiceStatus, StatusHandle},
        ServiceCore,
    },
    DynError,
};

pub struct ServiceRunnerHandle<Message, Settings, State, StateOperator, RuntimeServiceId> {
    service_handle: ServiceHandle<Message, Settings, State, StateOperator, RuntimeServiceId>,
    runner_join_handle: JoinHandle<()>,
}

impl<Message, Settings, State, StateOperator, RuntimeServiceId>
    ServiceRunnerHandle<Message, Settings, State, StateOperator, RuntimeServiceId>
{
    pub const fn service_handle(
        &self,
    ) -> &ServiceHandle<Message, Settings, State, StateOperator, RuntimeServiceId> {
        &self.service_handle
    }

    pub const fn runner_join_handle(&self) -> &JoinHandle<()> {
        &self.runner_join_handle
    }
}

/// Executor for a `Service`.
///
/// Contains all the necessary information to run a `Service`.
pub struct ServiceRunner<Message, Settings, State, StateOperator, RuntimeServiceId> {
    service_resources: ServiceResources<Settings, State, RuntimeServiceId>,
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
    StateOp: StateOperator<State = State>,
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
    Message: 'static + Send,
    Settings: Clone + 'static + Sync + Send,
    State: ServiceState<Settings = Settings> + Clone + Send + Sync + 'static,
    <State as ServiceState>::Error: Display,
    StateOp: StateOperator<State = State> + Send + 'static,
    RuntimeServiceId: 'static + Clone + Send,
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
    pub fn run<Service>(
        self,
    ) -> ServiceRunnerHandle<Message, Settings, State, StateOp, RuntimeServiceId>
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
    {
        let service_handle = ServiceHandle::from(&self);
        let runtime = self.service_resources.overwatch_handle.runtime().clone();
        let runner_join_handle = runtime.spawn(self.run_::<Service>());

        ServiceRunnerHandle {
            service_handle,
            runner_join_handle,
        }
    }

    async fn run_<Service>(self)
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
    {
        let Self {
            service_resources,
            state_handle,
            mut lifecycle_handle,
            relay,
            relay_buffer_size,
            status_handle,
            ..
        } = self;

        let Relay {
            inbound,
            outbound: _,
            consumer_sender,
            consumer_receiver,
        } = relay;

        let runtime = service_resources.overwatch_handle.runtime().clone();
        let mut service_task_handle: Option<_> = None;
        let mut state_handle_task_handle: Option<_> = None;

        let mut inbound_relay = Some(inbound);

        while let Some(lifecycle_message) = lifecycle_handle.next().await {
            match lifecycle_message {
                LifecycleMessage::Start(sender) => {
                    if !status_handle.borrow().is_startable() {
                        info!("Service is already running.");
                        // TODO: Sending a different signal could be very handy to
                        //  indicate that the service is already running.
                        sender
                            .send(())
                            .expect("Failed sending the Start FinishedSignal.");
                        continue;
                    }
                    Self::handle_start::<Service>(
                        &runtime,
                        &service_resources,
                        inbound_relay.take().expect("Inbound relay must exist."),
                        state_handle.clone(),
                        &mut service_task_handle,
                        &mut state_handle_task_handle,
                        &sender,
                    );
                }
                LifecycleMessage::Stop(sender) => {
                    if !status_handle.borrow().is_stoppable() {
                        info!("Service is already stopped.");
                        // TODO: Sending a different signal could be very handy to
                        //  indicate that the service is already stopped.
                        sender
                            .send(())
                            .expect("Failed sending the Stop FinishedSignal.");
                        continue;
                    }
                    let received_inbound_relay = Self::handle_stop(
                        &mut service_task_handle,
                        &mut state_handle_task_handle,
                        &service_resources,
                        &consumer_receiver,
                        consumer_sender.clone(),
                        &sender,
                        relay_buffer_size,
                    );
                    inbound_relay = Some(received_inbound_relay);
                }
            }
        }
    }

    fn handle_start<Service>(
        runtime: &Handle,
        service_resources: &ServiceResources<Settings, State, RuntimeServiceId>,
        inbound_relay: InboundRelay<Message>,
        state_handle: StateHandle<State, StateOp>,
        service_task_handle: &mut Option<JoinHandle<Result<(), DynError>>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        sender: &Sender<FinishedSignal>,
    ) where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
    {
        let initial_state = match Self::get_service_initial_state(service_resources) {
            Ok(initial_state) => initial_state,
            Err(error) => {
                panic!("Failed to create initial state from settings: {error}");
            }
        };

        let services_resources_handle = service_resources.to_handle(inbound_relay);
        let service = Service::init(services_resources_handle, initial_state.clone());
        service_resources.state_updater.update(initial_state);

        match service {
            Ok(service) => {
                Self::handle_service_run(
                    service,
                    runtime,
                    service_resources,
                    state_handle,
                    service_task_handle,
                    state_handle_task_handle,
                    sender,
                );
            }
            Err(error) => {
                panic!("Error while initialising service: {error}");
            }
        }
    }

    fn handle_service_run<Service>(
        service: Service,
        runtime: &Handle,
        service_resources: &ServiceResources<Settings, State, RuntimeServiceId>,
        state_handle: StateHandle<State, StateOp>,
        service_task_handle: &mut Option<JoinHandle<Result<(), DynError>>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        sender: &Sender<FinishedSignal>,
    ) where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
    {
        *service_task_handle = Some(runtime.spawn(service.run()));
        *state_handle_task_handle = Some(runtime.spawn(state_handle.run()));

        sender
            .send(())
            .expect("Failed sending the Start FinishedSignal.");

        service_resources
            .status_handle
            .updater()
            .update(ServiceStatus::Running);
    }

    fn handle_stop(
        service_task_handle: &mut Option<JoinHandle<Result<(), DynError>>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        service_resources: &ServiceResources<Settings, State, RuntimeServiceId>,
        consumer_receiver: &ConsumerReceiver<Message>,
        consumer_sender: ConsumerSender<Message>,
        stop_finished_signal_sender: &Sender<FinishedSignal>,
        relay_buffer_size: usize,
    ) -> InboundRelay<Message> {
        Self::stop_service(service_task_handle, state_handle_task_handle);
        service_resources
            .status_handle
            .updater()
            .update(ServiceStatus::Stopped);
        let consumer = consumer_receiver
            .recv()
            .expect("Consumer must be retrieved.");
        let inbound_relay = InboundRelay::new(consumer, consumer_sender, relay_buffer_size);
        stop_finished_signal_sender
            .send(())
            .expect("Failed sending the Stop FinishedSignal.");
        inbound_relay
    }

    /// Retrieves the initial state for the service.
    ///
    /// First tries to load the state from the operator (a previously saved
    /// state). If it fails, it defaults to the initial state created from
    /// the settings.
    fn get_service_initial_state(
        service_resources: &ServiceResources<Settings, State, RuntimeServiceId>,
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
