use std::{any::Any, fmt::Display};

use tokio::task::{JoinError, JoinHandle};
use tokio_stream::StreamExt as _;
use tracing::{debug, error, info};

use crate::{
    DynError,
    overwatch::{ServicePanic, handle::OverwatchHandle},
    services::{
        AsServiceId, ServiceCore,
        lifecycle::LifecycleMessage,
        resources::ServiceResources,
        runner::ServiceRunnerHandle,
        service_handle::ServiceHandle,
        state::{ServiceState, StateOperator},
    },
};

type ServiceTaskHandle = JoinHandle<Result<(), DynError>>;
/// How the `Service` task ended: the value returned by the `Service`, or the
/// reason why its task didn't complete (a panic, or being aborted).
type ServiceTaskResult = Result<Result<(), DynError>, JoinError>;

/// Extracts the message from the payload of a panic.
///
/// `panic!` produces either a `&str` or a `String`. Anything else comes from
/// `std::panic::panic_any`.
fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "Non-string panic payload".to_owned())
}

#[derive(Clone, Copy)]
struct TaskNames {
    service: &'static str,
    state: &'static str,
}

#[expect(
    unexpected_cfgs,
    reason = "tokio_unstable is supplied externally through RUSTFLAGS"
)]
fn spawn_task<T>(
    runtime: &tokio::runtime::Handle,
    name: Option<&'static str>,
    future: impl Future<Output = T> + Send + 'static,
) -> JoinHandle<T>
where
    T: Send + 'static,
{
    #[cfg(all(feature = "tokio-task-names", tokio_unstable))]
    {
        if let Some(name) = name {
            return tokio::task::Builder::new()
                .name(name)
                .spawn_on(future, runtime)
                .expect("failed to spawn named Overwatch task");
        }

        runtime.spawn(future)
    }

    #[cfg(not(all(feature = "tokio-task-names", tokio_unstable)))]
    {
        let _ = name;
        runtime.spawn(future)
    }
}

#[derive(PartialEq, Eq)]
enum ServiceLifecyclePhase {
    Started,
    Stopped,
}

/// Executor for a `Service`.
///
/// Contains all the necessary information to run a `Service`.
pub struct ServiceRunner<Message, Settings, State, StateOperator, RuntimeServiceId> {
    service_resources: ServiceResources<Message, Settings, State, StateOperator, RuntimeServiceId>,
    service_lifecycle_phase: ServiceLifecyclePhase,
}

impl<Message, Settings, State, StateOp, RuntimeServiceId>
    ServiceRunner<Message, Settings, State, StateOp, RuntimeServiceId>
where
    Settings: Clone,
    State: ServiceState<Settings = Settings> + Clone,
    StateOp: StateOperator<RuntimeServiceId, State = State>,
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
            service_lifecycle_phase: ServiceLifecyclePhase::Stopped,
        }
    }
}

#[expect(
    unexpected_cfgs,
    reason = "tokio_unstable is supplied externally through RUSTFLAGS"
)]
impl<Message, Settings, State, StateOp, RuntimeServiceId>
    ServiceRunner<Message, Settings, State, StateOp, RuntimeServiceId>
where
    Message: 'static + Send,
    Settings: Clone + 'static + Sync + Send,
    State: ServiceState<Settings = Settings> + Clone + Send + Sync + 'static,
    <State as ServiceState>::Error: Display,
    StateOp: StateOperator<RuntimeServiceId, State = State> + Send + 'static,
    <StateOp as StateOperator<RuntimeServiceId>>::LoadError: Display,
    RuntimeServiceId: 'static + Clone + Send,
{
    /// Spawn the `ServiceRunner` loop. This will listen for lifecycle messages
    /// and act upon them.
    ///
    /// # Returns
    ///
    /// A [`ServiceRunnerHandle`] that contains the [`ServiceHandle`] and the
    /// [`JoinHandle`] of the [`ServiceRunner`] task.
    #[cfg(all(feature = "tokio-task-names", tokio_unstable))]
    pub fn run<Service>(
        self,
    ) -> ServiceRunnerHandle<Message, Settings, State, StateOp, RuntimeServiceId>
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
        RuntimeServiceId: AsServiceId<Service> + Display + Sync + crate::services::ServiceTaskNames,
    {
        let service_id = <RuntimeServiceId as AsServiceId<Service>>::SERVICE_ID;
        let task_names = TaskNames {
            service: service_id.service_task_name(),
            state: service_id.state_task_name(),
        };

        self.spawn_runner::<Service>(Some(task_names))
    }

    /// Spawn the `ServiceRunner` loop. This will listen for lifecycle messages
    /// and act upon them.
    ///
    /// # Returns
    ///
    /// A [`ServiceRunnerHandle`] that contains the [`ServiceHandle`] and the
    /// [`JoinHandle`] of the [`ServiceRunner`] task.
    #[cfg(not(all(feature = "tokio-task-names", tokio_unstable)))]
    pub fn run<Service>(
        self,
    ) -> ServiceRunnerHandle<Message, Settings, State, StateOp, RuntimeServiceId>
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
        RuntimeServiceId: AsServiceId<Service> + Display + Sync,
    {
        self.spawn_runner::<Service>(None)
    }

    fn spawn_runner<Service>(
        self,
        task_names: Option<TaskNames>,
    ) -> ServiceRunnerHandle<Message, Settings, State, StateOp, RuntimeServiceId>
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
        RuntimeServiceId: AsServiceId<Service> + Display + Sync,
    {
        let service_handle = ServiceHandle::from(&self.service_resources);
        let runtime = self.service_resources.overwatch_handle().runtime().clone();
        let runner_join_handle = runtime.spawn(self.run_::<Service>(task_names));

        ServiceRunnerHandle::new(service_handle, runner_join_handle)
    }

    async fn run_<Service>(self, task_names: Option<TaskNames>)
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
        RuntimeServiceId: AsServiceId<Service> + Display + Sync,
    {
        let Self {
            mut service_resources,
            mut service_lifecycle_phase,
        } = self;

        // Handles to hold the Service and StateHandle tasks
        let mut service_task_handle: Option<_> = None;
        let mut state_handle_task_handle: Option<_> = None;

        loop {
            tokio::select! {
                lifecycle_message = service_resources.lifecycle_handle_mut().next() => {
                    let Some(lifecycle_message) = lifecycle_message else {
                        break;
                    };
                    match lifecycle_message {
                        LifecycleMessage::Start(finished_signal_sender) => {
                            if service_lifecycle_phase == ServiceLifecyclePhase::Started {
                                info!("Service is already running.");
                            } else {
                                if let Err(error) = Self::handle_start::<Service>(
                                    &mut service_resources,
                                    &mut service_task_handle,
                                    &mut state_handle_task_handle,
                                    task_names,
                                ) {
                                    error!(error, "Failed to start service.");
                                    continue;
                                }
                                service_lifecycle_phase = ServiceLifecyclePhase::Started;
                            }

                            // TODO: Sending a different signal could be handy to differentiate whether
                            //  the service was already started or not.
                            if let Err(error) = finished_signal_sender.send(()) {
                                debug!(
                                    "Error while sending the LifecycleMessage::Start signal: {error:?}.",
                                );
                            }
                        }
                        LifecycleMessage::Stop(finished_signal_sender) => {
                            if service_lifecycle_phase == ServiceLifecyclePhase::Stopped {
                                info!("Service is already stopped.");
                            } else {
                                Self::handle_stop::<Service>(
                                    &mut service_task_handle,
                                    &mut state_handle_task_handle,
                                    &mut service_resources,
                                )
                                .await;
                                service_lifecycle_phase = ServiceLifecyclePhase::Stopped;
                            }

                            // TODO: Sending a different signal could be handy to differentiate whether
                            //  the service was already stopped or not.
                            if let Err(error) = finished_signal_sender.send(()) {
                                debug!(
                                    "Error while sending the LifecycleMessage::Stop finished signal: {error:?}.",
                                );
                            }
                        }
                    }
                }
                // The `Service` finished on its own, whether by returning or by panicking.
                service_task_result = Self::wait_for_service_task(&mut service_task_handle) => {
                    Self::handle_service_finished::<Service>(
                        service_task_result,
                        &mut state_handle_task_handle,
                        &mut service_resources,
                    )
                    .await;
                    service_lifecycle_phase = ServiceLifecyclePhase::Stopped;
                }
            }
        }
    }

    /// Waits for the `Service` task to finish. It never resolves while there is
    /// no `Service` task.
    async fn wait_for_service_task(
        service_task_handle: &mut Option<ServiceTaskHandle>,
    ) -> ServiceTaskResult {
        let Some(service_join_handle) = service_task_handle.as_mut() else {
            return std::future::pending().await;
        };
        let service_task_result = service_join_handle.await;
        *service_task_handle = None;
        service_task_result
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
        service_task_handle: &mut Option<ServiceTaskHandle>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        task_names: Option<TaskNames>,
    ) -> Result<(), String>
    where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: Clone,
    {
        let initial_state = service_resources
            .get_service_initial_state()
            .map_err(|error| format!("Failed to create the initial state: {error}"))?;

        let service_resources_handle = service_resources.as_handle().unwrap_or_else(|error| {
            panic!("Failed to create the ServiceResourcesHandle: {error}");
        });
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
            .status_handle()
            .service_runner_updater()
            .notify_starting();

        Self::start_tasks(
            service,
            service_resources,
            service_task_handle,
            state_handle_task_handle,
            task_names,
        );

        Ok(())
    }

    fn start_tasks<Service>(
        service: Service,
        service_resources: &ServiceResources<Message, Settings, State, StateOp, RuntimeServiceId>,
        service_task_handle: &mut Option<ServiceTaskHandle>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        task_names: Option<TaskNames>,
    ) where
        Service: ServiceCore<RuntimeServiceId, Settings = Settings, State = State, Message = Message>
            + 'static,
        StateOp: StateOperator<RuntimeServiceId, State = State> + Clone,
    {
        let runtime = service_resources.overwatch_handle().runtime().clone();
        let service_task = service.run();
        *service_task_handle = Some(spawn_task(
            &runtime,
            task_names.map(|task_names| task_names.service),
            service_task,
        ));
        let state_handle_task = service_resources.state_handle().clone().run();
        *state_handle_task_handle = Some(spawn_task(
            &runtime,
            task_names.map(|task_names| task_names.state),
            state_handle_task,
        ));
    }

    /// Handles a [`LifecycleMessage::Stop`] event, ensuring proper shutdown and
    /// cleanup:
    ///
    /// - A `fuse` is sent to the
    ///   [`StateHandle`](crate::services::state::StateHandle), so its task is
    ///   gracefully stopped.
    /// - The `Service` task is aborted.
    /// - Final cleanup is performed.
    async fn handle_stop<Service>(
        service_task_handle: &mut Option<ServiceTaskHandle>,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        service_resources: &mut ServiceResources<
            Message,
            Settings,
            State,
            StateOp,
            RuntimeServiceId,
        >,
    ) where
        RuntimeServiceId: AsServiceId<Service> + Display + Sync,
    {
        Self::stop_state_handle_task(service_resources, state_handle_task_handle).await;
        let service_task_result = Self::stop_service_task(service_task_handle).await;
        Self::finish_stop::<Service>(service_task_result, service_resources);
    }

    /// Handles a `Service` that finished execution on its own, whether by
    /// returning or by panicking:
    ///
    /// - The `Service` task is already stopped.
    /// - A `fuse` is sent to the
    ///   [`StateHandle`](crate::services::state::StateHandle), so its task is
    ///   gracefully stopped.
    /// - Final cleanup is performed.
    async fn handle_service_finished<Service>(
        service_task_result: ServiceTaskResult,
        state_handle_task_handle: &mut Option<JoinHandle<()>>,
        service_resources: &mut ServiceResources<
            Message,
            Settings,
            State,
            StateOp,
            RuntimeServiceId,
        >,
    ) where
        RuntimeServiceId: AsServiceId<Service> + Display + Sync,
    {
        Self::stop_state_handle_task(service_resources, state_handle_task_handle).await;
        Self::finish_stop::<Service>(service_task_result, service_resources);
    }

    /// Final cleanup once the `Service` and `StateHandle` tasks are stopped.
    ///
    /// The status becomes
    /// [`ServiceStatus::Stopped`](crate::services::status::ServiceStatus::Stopped),
    /// or
    /// [`ServiceStatus::Failed`](crate::services::status::ServiceStatus::Failed)
    /// if the `Service` panicked. In that case the
    /// [`PanicPolicy`](crate::overwatch::PanicPolicy) is called.
    fn finish_stop<Service>(
        service_task_result: ServiceTaskResult,
        service_resources: &mut ServiceResources<
            Message,
            Settings,
            State,
            StateOp,
            RuntimeServiceId,
        >,
    ) where
        RuntimeServiceId: AsServiceId<Service> + Display + Sync,
    {
        service_resources
            .rebuild_inbound_relay()
            .unwrap_or_else(|error| {
                panic!("Could not rebuild the InboundRelay: {error}");
            });

        let panic_payload = match service_task_result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => {
                error!("Error while waiting for Service's task to be completed: {error}");
                None
            }
            // If it's not a panic, the task was aborted by a Stop.
            Err(join_error) => join_error.try_into_panic().ok(),
        };

        let status_updater = service_resources.status_handle().service_runner_updater();
        let Some(panic_payload) = panic_payload else {
            status_updater.notify_stopped();
            return;
        };

        let service_panic = ServicePanic::new(
            <RuntimeServiceId as AsServiceId<Service>>::SERVICE_ID,
            panic_message(panic_payload.as_ref()),
        );
        error!("{service_panic}");
        status_updater.notify_failed();
        Self::call_panic_policy(service_panic, service_resources.overwatch_handle());
    }

    /// Calls the [`PanicPolicy`](crate::overwatch::PanicPolicy) in its own
    /// task.
    ///
    /// It can't be awaited here: a policy that stops this `Service` (for
    /// example by shutting Overwatch down) needs this `ServiceRunner` to keep
    /// handling lifecycle messages.
    fn call_panic_policy(
        service_panic: ServicePanic<RuntimeServiceId>,
        overwatch_handle: &OverwatchHandle<RuntimeServiceId>,
    ) where
        RuntimeServiceId: Sync,
    {
        let Some(panic_policy) = overwatch_handle.panic_policy().cloned() else {
            return;
        };
        let overwatch_handle = overwatch_handle.clone();
        overwatch_handle.runtime().clone().spawn(async move {
            panic_policy
                .on_service_panic(service_panic, &overwatch_handle)
                .await;
        });
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

    /// Stops the `Service` task, aborting it if it's still running.
    async fn stop_service_task(
        service_task_handle: &mut Option<ServiceTaskHandle>,
    ) -> ServiceTaskResult {
        let Some(service_join_handle) = service_task_handle.take() else {
            panic!("ServiceTask_handle's JoinHandle must exist.");
        };
        if service_join_handle.is_finished() {
            return service_join_handle.await;
        }
        service_join_handle.abort_handle().abort();
        let service_task_result = service_join_handle.await;
        info!("Service task aborted.");
        service_task_result
    }
}
