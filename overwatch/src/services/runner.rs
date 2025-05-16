use std::fmt::Display;

use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::info;

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        handle::ServiceHandle,
        life_cycle::LifecycleMessage,
        resources::ServiceResources,
        state::{ServiceState, StateOperator},
        status::ServiceStatus,
        ServiceCore,
    },
    utils::finished_signal::Sender,
    DynError,
};

pub struct ServiceRunnerHandle<Message, Settings, State, StateOperator> {
    service_handle: ServiceHandle<Message, Settings, State, StateOperator>,
    runner_join_handle: JoinHandle<()>,
}

impl<Message, Settings, State, StateOperator>
    ServiceRunnerHandle<Message, Settings, State, StateOperator>
{
    pub const fn service_handle(&self) -> &ServiceHandle<Message, Settings, State, StateOperator> {
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
    service_resources: ServiceResources<Message, Settings, State, StateOperator, RuntimeServiceId>,
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
        let service_resources =
            ServiceResources::new(settings, overwatch_handle, relay_buffer_size);
        Self { service_resources }
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
    /// Spawn the `ServiceRunner` loop. This will listen for lifecycle messages
    /// and act upon them.
    ///
    /// # Returns
    ///
    /// A [`ServiceRunnerHandle`] that contains the [`ServiceHandle`] and the
    /// [`JoinHandle`] of the [`ServiceRunner`] task.
    pub fn run<Service>(self) -> ServiceRunnerHandle<Message, Settings, State, StateOp>
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
    {
        let service_handle = ServiceHandle::from(&self.service_resources);
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
        let mut service_resources = self.service_resources;

        // Handles to hold the Service and StateHandle tasks
        let mut service_task_handle: Option<_> = None;
        let mut state_handle_task_handle: Option<_> = None;

        while let Some(lifecycle_message) = service_resources.lifecycle_handle.next().await {
            match lifecycle_message {
                LifecycleMessage::Start(finished_signal_sender) => {
                    if !service_resources.status_handle.borrow().is_startable() {
                        info!("Service is already running.");
                        // TODO: Sending a different signal could be very handy to
                        //  indicate that the service is already running.
                        finished_signal_sender
                            .send(())
                            .expect("Failed to send the Start FinishedSignal.");
                        continue;
                    }
                    Self::handle_start::<Service>(
                        &mut service_resources,
                        &mut service_task_handle,
                        &mut state_handle_task_handle,
                        finished_signal_sender,
                    );
                }
                LifecycleMessage::Stop(finished_signal_sender) => {
                    if !service_resources.status_handle.borrow().is_stoppable() {
                        info!("Service is already stopped.");
                        // TODO: Sending a different signal could be very handy to
                        //  indicate that the service is already stopped.
                        finished_signal_sender
                            .send(())
                            .expect("Failed to send the Stop FinishedSignal.");
                        continue;
                    }
                    Self::handle_stop(
                        &mut service_task_handle,
                        &mut state_handle_task_handle,
                        &mut service_resources,
                        finished_signal_sender,
                    );
                }
            }
        }
    }

    fn handle_start<Service>(
        service_resources: &mut ServiceResources<
            Message,
            Settings,
            State,
            StateOp,
            RuntimeServiceId,
        >,
        service_task_handle: &mut Option<JoinHandle<Result<(), DynError>>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        finished_signal_sender: Sender,
    ) where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
    {
        let initial_state = match service_resources.get_service_initial_state() {
            Ok(initial_state) => initial_state,
            Err(error) => {
                panic!("Failed to create the initial state from settings: {error}");
            }
        };

        let inbound_relay = service_resources
            .inbound_relay
            .take()
            .expect("Failed to retrieve inbound relay.");
        let service_resources_handle = service_resources.to_handle(inbound_relay);
        let service = Service::init(service_resources_handle, initial_state.clone());

        match service {
            Ok(service) => {
                service_resources.state_updater.update(Some(initial_state));
                Self::handle_service_run(
                    service,
                    service_resources,
                    service_task_handle,
                    state_handle_task_handle,
                    finished_signal_sender,
                );
            }
            Err(error) => {
                panic!("Error while initialising service: {error}");
            }
        }
    }

    fn handle_service_run<Service>(
        service: Service,
        service_resources: &ServiceResources<Message, Settings, State, StateOp, RuntimeServiceId>,
        service_task_handle: &mut Option<JoinHandle<Result<(), DynError>>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        finished_signal_sender: Sender,
    ) where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: StateOperator<State = State> + Clone,
    {
        let runtime = service_resources.overwatch_handle.runtime();
        let state_handle = service_resources.state_handle.clone();
        *service_task_handle = Some(runtime.spawn(service.run()));
        *state_handle_task_handle = Some(runtime.spawn(state_handle.run()));

        service_resources
            .status_handle
            .updater()
            .update(ServiceStatus::Running);

        finished_signal_sender
            .send(())
            .expect("Failed to send the Start FinishedSignal.");
    }

    fn handle_stop(
        service_task_handle: &mut Option<JoinHandle<Result<(), DynError>>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        service_resources: &mut ServiceResources<
            Message,
            Settings,
            State,
            StateOp,
            RuntimeServiceId,
        >,
        finished_signal_sender: Sender,
    ) {
        Self::stop_service(service_task_handle, state_handle_task_handle);
        service_resources
            .status_handle
            .updater()
            .update(ServiceStatus::Stopped);
        service_resources
            .retrieve_inbound_relay_consumer()
            .unwrap_or_else(|error| {
                panic!("Failed to retrieve inbound relay consumer: {error}");
            });
        finished_signal_sender
            .send(())
            .expect("Failed to send the Stop FinishedSignal.");
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
