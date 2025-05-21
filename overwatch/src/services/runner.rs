use std::{fmt::Display, future::Future};

use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::{error, info};

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        handle::ServiceHandle,
        life_cycle::{LifecycleMessage, LifecyclePhase},
        resources::ServiceResources,
        state::{ServiceState, StateOperator},
        ServiceCore,
    },
    utils::finished_signal,
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

    pub fn runner_join_handle_owned(self) -> JoinHandle<()> {
        self.runner_join_handle
    }
}

/// Executor for a `Service`.
///
/// Contains all the necessary information to run a `Service`.
pub struct ServiceRunner<Message, Settings, State, StateOperator, RuntimeServiceId> {
    service_resources: ServiceResources<Message, Settings, State, StateOperator, RuntimeServiceId>,
    service_lifecycle_phase: LifecyclePhase,
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
        Self {
            service_resources,
            service_lifecycle_phase: LifecyclePhase::Stopped,
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
        let Self {
            mut service_resources,
            mut service_lifecycle_phase,
        } = self;

        // Handles to hold the Service and StateHandle tasks
        let mut service_task_handle: Option<_> = None;
        let mut state_handle_task_handle: Option<_> = None;

        while let Some(lifecycle_message) = service_resources.lifecycle_handle.next().await {
            match lifecycle_message {
                LifecycleMessage::Start(finished_signal_sender) => {
                    if service_lifecycle_phase == LifecyclePhase::Started {
                        info!("Service is already running.");
                    } else {
                        Self::handle_start::<Service>(
                            &mut service_resources,
                            &mut service_task_handle,
                            &mut state_handle_task_handle,
                        );
                        service_lifecycle_phase = LifecyclePhase::Started;
                    }

                    // TODO: Sending a different signal could be handy to differentiate whether
                    //  the service was already started or not.
                    if let Err(error) = finished_signal_sender.send(()) {
                        dbg!(
                            "Error while sending the LifecycleMessage::Start signal: {}.",
                            error
                        );
                    }
                }
                LifecycleMessage::Stop(finished_signal_sender) => {
                    if service_lifecycle_phase == LifecyclePhase::Stopped {
                        info!("Service is already stopped.");
                    } else {
                        Self::handle_stop(
                            &mut service_task_handle,
                            &mut state_handle_task_handle,
                            &mut service_resources,
                        )
                        .await;
                        service_lifecycle_phase = LifecyclePhase::Stopped;
                    }

                    // TODO: Sending a different signal could be handy to differentiate whether
                    //  the service was already stopped or not.
                    if let Err(error) = finished_signal_sender.send(()) {
                        dbg!("Error while sending the LifecycleMessage::Stop finished signal: {}. Likely due to the receiver being already dropped in the Service::run task.", error);
                    }
                }
            }
        }
    }

    /// Handles a [`LifecycleMessage::Start`] event, ensuring the `Service` task
    /// and its corresponding `StateHandle` task are both started correctly.
    fn handle_start<Service>(
        service_resources: &mut ServiceResources<
            Message,
            Settings,
            State,
            StateOp,
            RuntimeServiceId,
        >,
        service_task_handle: &mut Option<JoinHandle<()>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
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
        let service = match Service::init(service_resources_handle, initial_state.clone()) {
            Ok(service) => service,
            Err(error) => {
                panic!("Service couldn't be initialised: {error}");
            }
        };

        service_resources
            .state_updater()
            .update(Some(initial_state));

        service_resources
            .status_handle
            .service_runner_updater()
            .notify_starting();

        Self::start_tasks(
            service,
            service_resources,
            service_task_handle,
            state_handle_task_handle,
        );
    }

    fn start_tasks<Service>(
        service: Service,
        service_resources: &ServiceResources<Message, Settings, State, StateOp, RuntimeServiceId>,
        service_task_handle: &mut Option<JoinHandle<()>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
    ) where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: StateOperator<State = State> + Clone,
    {
        let runtime = service_resources.overwatch_handle.runtime().clone();
        let service_task = Self::create_service_run_task(service, service_resources);
        *service_task_handle = Some(runtime.spawn(service_task));
        let state_handle_task = service_resources.state_handle.clone().run();
        *state_handle_task_handle = Some(runtime.spawn(state_handle_task));
    }

    fn create_service_run_task<Service>(
        service: Service,
        service_resources: &ServiceResources<Message, Settings, State, StateOp, RuntimeServiceId>,
    ) -> impl Future<Output = ()>
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
    {
        let task = service.run();
        let lifecycle_notifier = service_resources.lifecycle_handle.notifier().clone();

        // Receiver is ignored because it's pointless:
        // - If we wait for it, the Stop message will eventually abort it before the
        //   finished signal is received.
        // - If we don't wait for it and the task finishes, the ServiceRunner will
        //   ignore it.
        let (sender, _receiver) = finished_signal::channel();

        // When the `Service`'s task finishes, a [`LifecycleMessage::Stop`] is sent to
        // the `ServiceRunner` to ensure proper cleanup.
        async move {
            if let Err(error) = task.await {
                error!("Error while waiting for Service's task to be completed: {error}");
            }
            if let Err(error) = lifecycle_notifier
                .send(LifecycleMessage::Stop(sender))
                .await
            {
                error!("Error while sending a Stop to the ServiceRunner: {error}");
            }
        }
    }

    /// Handles a [`LifecycleMessage::Stop`] event, ensuring proper shutdown and
    /// cleanup.
    ///
    /// This can occur in two scenarios:
    ///
    /// 1. **User-initiated stop**: The user sends a stop message. In this case:
    /// - A `fuse` is sent to The
    ///   [`StatusHandle`](crate::services::status::StatusHandle), so its task
    ///   is gracefully stopped.
    /// - The `Service` task is aborted.
    /// - Final cleanup is performed.
    ///
    /// 2. **Service self-termination**: The `Service` finishes execution on its
    ///    own. In this case:
    /// - The `Service` task is already stopped.
    /// - A `fuse` is sent to the
    ///   [`StatusHandle`](crate::services::status::StatusHandle), so its task
    ///   is gracefully stopped.
    /// - Final cleanup is performed.
    ///
    /// This ensures both tasks are properly stopped and cleaned up.
    async fn handle_stop(
        service_task_handle: &mut Option<JoinHandle<()>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        service_resources: &mut ServiceResources<
            Message,
            Settings,
            State,
            StateOp,
            RuntimeServiceId,
        >,
    ) {
        Self::stop_tasks(
            service_resources,
            service_task_handle,
            state_handle_task_handle,
        )
        .await;

        service_resources
            .retrieve_inbound_relay_consumer()
            .unwrap_or_else(|error| {
                panic!("Failed to retrieve inbound relay consumer: {error}");
            });

        service_resources
            .status_handle
            .service_runner_updater()
            .notify_stopped();
    }

    async fn stop_tasks(
        service_resources: &mut ServiceResources<
            Message,
            Settings,
            State,
            StateOp,
            RuntimeServiceId,
        >,
        service_task_handle: &mut Option<JoinHandle<()>>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
    ) {
        Self::stop_state_handle_task(service_resources, state_handle_task_handle).await;
        Self::stop_service_task(service_task_handle).await;
    }

    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "Forces `service_resources` only have one reference."
    )]
    async fn stop_state_handle_task(
        service_resources: &mut ServiceResources<
            Message,
            Settings,
            State,
            StateOp,
            RuntimeServiceId,
        >,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
    ) {
        let Some(state_handle_join_handle) = state_handle_task_handle.take() else {
            panic!("StateHandle's JoinHandle must exist.");
        };
        if !state_handle_join_handle.is_finished() {
            let operator_fuse_sender = service_resources.operator_fuse_sender();
            if let Err(error) = operator_fuse_sender.send(()) {
                error!("Error while sending fuse: {error}");
            }
            let _ = state_handle_join_handle.await;
            info!("StateHandle task aborted.");
        }
    }

    async fn stop_service_task(service_task_handle: &mut Option<JoinHandle<()>>) {
        let Some(service_join_handle) = service_task_handle.take() else {
            panic!("ServiceTask_handle's JoinHandle must exist.");
        };
        if !service_join_handle.is_finished() {
            service_join_handle.abort_handle().abort();
            let _ = service_join_handle.await;
            info!("Service task aborted.");
        }
    }
}
