use std::fmt::Debug;

use tokio::{
    runtime::Handle,
    sync::{mpsc::Receiver, oneshot},
};
#[cfg(feature = "instrumentation")]
use tracing::instrument;
use tracing::{error, info};

use crate::{
    DynError,
    overwatch::{
        Error, Overwatch, OverwatchExit, ServicePanicPolicy, Services,
        commands::{
            OverwatchCommand, OverwatchManagementCommand, RelayCommand, ServiceAllCommand,
            ServiceLifecycleCommand, ServiceSequenceCommand, ServiceSingleCommand, SettingsCommand,
            StatusCommand,
        },
        handle::OverwatchHandle,
        runtime::OverwatchRuntime,
    },
    utils::{finished_signal, runtime::default_multithread_runtime},
};

/// Overwatch thread identifier.
///
/// It's used for creating the [`Runtime`](tokio::runtime::Runtime) that
/// Overwatch uses internally.
pub const OVERWATCH_THREAD_NAME: &str = "Overwatch";

/// Handle a running [`Overwatch`].
///
/// It's one-shot: once it stops running, whether by a graceful shutdown or
/// otherwise, there is no mechanism to restart it. A new [`Overwatch`] instance
/// must be created instead. This implies that existing services, relays and
/// other allocated resources will be dropped.
///
/// It only contains what's required to run [`Overwatch`] as a main loop and to
/// be able to stop it.
///
/// That is, it's responsible for [`Overwatch`]'s application lifecycle.
pub struct GenericOverwatchRunner<Services, RuntimeServiceId> {
    services: Services,
    service_panic_policy: ServicePanicPolicy,
    finish_signal_sender: oneshot::Sender<OverwatchExit<RuntimeServiceId>>,
    commands_receiver: Receiver<OverwatchCommand<RuntimeServiceId>>,
}

/// Shorthand for [`GenericOverwatchRunner`]
pub type OverwatchRunner<ServicesImpl> =
    GenericOverwatchRunner<ServicesImpl, <ServicesImpl as Services>::RuntimeServiceId>;

impl<ServicesImpl> OverwatchRunner<ServicesImpl>
where
    ServicesImpl: Services + Send + 'static,
    ServicesImpl::RuntimeServiceId: Clone + Debug + Send,
{
    /// Start the Overwatch runner process.
    ///
    /// Create the [`Runtime`](tokio::runtime::Runtime), initialize the
    /// [`Services`] and start listening for [`Overwatch`] related tasks.
    ///
    /// Return the [`Overwatch`] instance that handles this runner.
    ///
    /// A panic in a `Service` is handled with the default
    /// [`ServicePanicPolicy`]. Use [`Self::run_with_panic_policy`] to choose a
    /// different one.
    ///
    /// # Errors
    ///
    /// If the runner process cannot be created.
    pub fn run(
        settings: ServicesImpl::Settings,
        handle: Option<Handle>,
    ) -> Result<Overwatch<ServicesImpl::RuntimeServiceId>, DynError> {
        Self::run_with_panic_policy(settings, handle, ServicePanicPolicy::default())
    }

    /// Same as [`Self::run`], choosing what happens when a `Service` panics.
    ///
    /// # Errors
    ///
    /// If the runner process cannot be created.
    pub fn run_with_panic_policy(
        settings: ServicesImpl::Settings,
        handle: Option<Handle>,
        service_panic_policy: ServicePanicPolicy,
    ) -> Result<Overwatch<ServicesImpl::RuntimeServiceId>, DynError> {
        let runtime = handle.map_or_else(
            || OverwatchRuntime::TokioRuntime(default_multithread_runtime()),
            OverwatchRuntime::TokioHandle,
        );

        let (finish_signal_sender, finish_runner_signal) = oneshot::channel();
        let (commands_sender, commands_receiver) = tokio::sync::mpsc::channel(16);
        let handle = OverwatchHandle::new(runtime.handle().clone(), commands_sender);
        let services = ServicesImpl::new(settings, handle.clone())?;

        let runner = Self {
            services,
            service_panic_policy,
            finish_signal_sender,
            commands_receiver,
        };

        runtime.handle().spawn(runner.run_());

        Ok(Overwatch {
            runtime,
            handle,
            finish_runner_signal,
        })
    }

    #[cfg_attr(
        feature = "instrumentation",
        instrument(name = "overwatch-run", skip_all)
    )]
    async fn run_(self) {
        let Self {
            mut services,
            service_panic_policy,
            finish_signal_sender,
            mut commands_receiver,
        } = self;
        let mut exit = OverwatchExit::Shutdown;
        while let Some(command) = commands_receiver.recv().await {
            info!(command = ?command, "Overwatch command received");
            match command {
                OverwatchCommand::Relay(relay_command) => {
                    Self::handle_relay_command(&mut services, relay_command);
                }
                OverwatchCommand::Status(status_command) => {
                    Self::handle_status_command(&services, status_command);
                }
                OverwatchCommand::ServiceLifecycle(service_lifecycle_command) => {
                    Self::handle_service_lifecycle_command(
                        &mut services,
                        service_lifecycle_command,
                    )
                    .await;
                }
                OverwatchCommand::OverwatchManagement(command) => match command {
                    OverwatchManagementCommand::RetrieveServiceIds(reply_channel) => {
                        if let Err(e) = reply_channel.reply(services.ids()) {
                            error!(error=?e, "Error replying with service IDs.");
                        }
                    }
                    OverwatchManagementCommand::Shutdown(sender) => {
                        Self::shutdown(services).await;
                        if let Err(error) = sender.send(()) {
                            error!(error=?error, "Error sending Shutdown finished signal.");
                        }
                        break;
                    }
                    OverwatchManagementCommand::ServicePanicked(service_panic) => {
                        match service_panic_policy {
                            ServicePanicPolicy::ShutdownOverwatch => {
                                error!(
                                    "Service {:?} panicked, shutting down Overwatch.",
                                    service_panic.service_id
                                );
                                Self::shutdown(services).await;
                                exit = OverwatchExit::ServicePanicked(service_panic);
                                break;
                            }
                            // The `ServiceRunner` stops the `Service` on its own.
                            ServicePanicPolicy::StopService => {}
                        }
                    }
                },
                OverwatchCommand::Settings(settings) => {
                    Self::handle_settings_command(&mut services, settings);
                }
            }
        }

        // Signal that we finished execution
        info!("OverwatchRunner finished execution, sending the finish signal.");
        finish_signal_sender
            .send(exit)
            .expect("Overwatch run finish signal to be sent properly");
    }

    /// Stop every `Service` and tear the [`Services`] down.
    async fn shutdown(mut services: ServicesImpl) {
        if let Err(error) = services.stop_all().await {
            error!(error=?error, "Error stopping all services during teardown.");
        }
        if let Err(error) = services.teardown().await {
            error!(error=?error, "Error tearing down services.");
        }
    }

    /// Handle a [`RelayCommand`].
    ///
    /// # Arguments
    ///
    /// * `services`: The [`Services`] instance to handle the command for.
    /// * `RelayCommand`: The command to handle.
    fn handle_relay_command(
        services: &mut ServicesImpl,
        RelayCommand {
            service_id,
            reply_channel,
        }: RelayCommand<ServicesImpl::RuntimeServiceId>,
    ) {
        if let Err(e) = reply_channel.reply(services.request_relay(&service_id)) {
            info!(error=?e, "Error requesting relay for service {service_id:#?}");
        }
    }

    /// Handle a [`StatusCommand`].
    ///
    /// # Arguments
    ///
    /// * `services`: The [`Services`] instance to handle the command for.
    /// * `SettingsCommand`: The command to handle.
    fn handle_settings_command(
        services: &mut ServicesImpl,
        SettingsCommand(settings): SettingsCommand,
    ) {
        let Ok(settings) = settings.downcast::<ServicesImpl::Settings>() else {
            unreachable!("Statically should always be of the correct type");
        };
        services.update_settings(*settings);
    }

    /// Handle a [`StatusCommand`].
    ///
    /// # Arguments
    ///
    /// * `services`: The [`Services`] instance to handle the command for.
    /// * `StatusCommand`: The command to handle.
    fn handle_status_command(
        services: &ServicesImpl,
        StatusCommand {
            service_id,
            reply_channel,
        }: StatusCommand<ServicesImpl::RuntimeServiceId>,
    ) {
        let watcher = services.request_status_watcher(&service_id);
        if reply_channel.reply(watcher).is_err() {
            error!("Error reporting back status watcher for service: {service_id:#?}");
        }
    }

    /// Handle a [`ServiceLifecycleCommand`].
    ///
    /// # Arguments
    ///
    /// * `services`: The [`Services`] instance to handle the command for.
    /// * `command`: The command to handle.
    ///
    /// # Notes
    ///
    /// * Because this method is async and takes a `ServicesImpl` reference, it
    ///   would need to propagate `Sync` traits. To avoid this, we use a `&mut
    ///   ServicesImpl` reference.
    async fn handle_service_lifecycle_command(
        services: &mut ServicesImpl,
        command: ServiceLifecycleCommand<ServicesImpl::RuntimeServiceId>,
    ) {
        match command {
            ServiceLifecycleCommand::StartService(ServiceSingleCommand { service_id, sender }) => {
                handle_service_lifecycle_command_operation(
                    services.start(&service_id),
                    sender,
                    "StartService",
                )
                .await;
            }
            ServiceLifecycleCommand::StartServiceSequence(ServiceSequenceCommand {
                service_ids,
                sender,
            }) => {
                handle_service_lifecycle_command_operation(
                    services.start_sequence(service_ids.as_slice()),
                    sender,
                    "StartServiceSequence",
                )
                .await;
            }
            ServiceLifecycleCommand::StartAllServices(ServiceAllCommand { sender }) => {
                handle_service_lifecycle_command_operation(
                    services.start_all(),
                    sender,
                    "StartAllServices",
                )
                .await;
            }
            ServiceLifecycleCommand::StopService(ServiceSingleCommand { service_id, sender }) => {
                handle_service_lifecycle_command_operation(
                    services.stop(&service_id),
                    sender,
                    "StopService",
                )
                .await;
            }
            ServiceLifecycleCommand::StopServiceSequence(ServiceSequenceCommand {
                service_ids,
                sender,
            }) => {
                handle_service_lifecycle_command_operation(
                    services.stop_sequence(service_ids.as_slice()),
                    sender,
                    "StopServiceSequence",
                )
                .await;
            }
            ServiceLifecycleCommand::StopAllServices(ServiceAllCommand { sender }) => {
                handle_service_lifecycle_command_operation(
                    services.stop_all(),
                    sender,
                    "StopAllServices",
                )
                .await;
            }
        }
    }
}

/// Handle a [`ServiceLifecycleCommand`] operation.
///
/// # Arguments
///
/// * `operation`: The operation to run. A future which should return a
///   `Result<(), Error>`.
/// * `sender`: The sender for the finished signal.
/// * `operation_name`: The name of the operation, used for logging purposes.
async fn handle_service_lifecycle_command_operation<F>(
    operation: F,
    sender: finished_signal::Sender,
    operation_name: &str,
) where
    F: Future<Output = Result<(), Error>> + Send,
{
    match operation.await {
        Ok(()) => {
            if let Err(error) = sender.send(()) {
                error!(error=?error, "Error while sending the finished signal for {operation_name} operation.");
            }
        }
        Err(error) => {
            error!(error=?error, "Error while running {operation_name} operation.");
        }
    }
}
