use std::fmt::Debug;

use tokio::{runtime::Runtime, sync::mpsc::Receiver};
#[cfg(feature = "instrumentation")]
use tracing::instrument;
use tracing::{error, info};

use crate::{
    overwatch::{
        commands::{
            OverwatchCommand, OverwatchLifecycleCommand, RelayCommand, ServiceAllCommand,
            ServiceLifecycleCommand, ServiceSequenceCommand, ServiceSingleCommand, SettingsCommand,
            StatusCommand,
        },
        handle::OverwatchHandle,
        Overwatch, Services,
    },
    utils::{finished_signal, runtime::default_multithread_runtime},
    DynError,
};

/// Overwatch thread identifier.
///
/// It's used for creating the [`Runtime`] that Overwatch uses internally.
pub const OVERWATCH_THREAD_NAME: &str = "Overwatch";

/// Handle a running [`Overwatch`].
///
/// It's usually one-shot.
///
/// It only contains what's required to run [`Overwatch`] as a main loop and to
/// be able to stop it.
///
/// That is, it's responsible for [`Overwatch`]'s application lifecycle.
pub struct GenericOverwatchRunner<Services, RuntimeServiceId> {
    services: Services,
    finish_signal_sender: finished_signal::Sender,
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
    /// Create the [`Runtime`], initialize the [`Services`] and start listening
    /// for [`Overwatch`] related tasks.
    ///
    /// Return the [`Overwatch`] instance that handles this runner.
    ///
    /// # Errors
    ///
    /// If the runner process cannot be created.
    pub fn run(
        settings: ServicesImpl::Settings,
        runtime: Option<Runtime>,
    ) -> Result<Overwatch<ServicesImpl::RuntimeServiceId>, DynError> {
        let runtime = runtime.unwrap_or_else(default_multithread_runtime);

        let (finish_signal_sender, finish_runner_signal) = finished_signal::channel();
        let (commands_sender, commands_receiver) = tokio::sync::mpsc::channel(16);
        let handle = OverwatchHandle::new(runtime.handle().clone(), commands_sender);
        let services = ServicesImpl::new(settings, handle.clone())?;

        let runner = Self {
            services,
            finish_signal_sender,
            commands_receiver,
        };

        runtime.spawn(runner.run_());

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
            finish_signal_sender,
            mut commands_receiver,
        } = self;
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
                OverwatchCommand::OverwatchLifecycle(command) => match command {
                    OverwatchLifecycleCommand::Shutdown(sender) => {
                        if let Err(error) = services.stop_all().await {
                            error!(error=?error, "Error stopping all services during teardown.");
                        }
                        if let Err(error) = services.teardown().await {
                            error!(error=?error, "Error tearing down services.");
                        }
                        if let Err(error) = sender.send(()) {
                            error!(error=?error, "Error sending Shutdown finished signal.");
                        }
                        break;
                    }
                },
                OverwatchCommand::Settings(settings) => {
                    Self::handle_settings_command(&mut services, settings);
                }
            }
        }

        // Signal that we finished execution
        info!("OverwatchRunner finished execution, sending finish signal.");
        finish_signal_sender
            .send(())
            .expect("Overwatch run finish signal to be sent properly");
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

    async fn handle_service_lifecycle_command(
        services: &mut ServicesImpl,
        command: ServiceLifecycleCommand<ServicesImpl::RuntimeServiceId>,
    ) {
        match command {
            ServiceLifecycleCommand::StartService(ServiceSingleCommand { service_id, sender }) => {
                if let Err(error) = services.start(&service_id).await {
                    error!(error=?error, "Error starting service: {service_id:#?}");
                }
                if let Err(error) = sender.send(()) {
                    error!(error=?error, "Error sending StartService finished signal.");
                }
            }
            ServiceLifecycleCommand::StartServiceSequence(ServiceSequenceCommand {
                service_ids,
                sender,
            }) => {
                if let Err(error) = services.start_sequence(service_ids.as_slice()).await {
                    error!(error=?error, "Error starting services: {service_ids:#?}");
                }
                if let Err(error) = sender.send(()) {
                    error!(error=?error, "Error sending StartServiceSequence finished signal.");
                }
            }
            ServiceLifecycleCommand::StartAllServices(ServiceAllCommand { sender }) => {
                if let Err(error) = services.start_all().await {
                    error!(error=?error, "Error starting all services.");
                }
                if let Err(error) = sender.send(()) {
                    error!(error=?error, "Error sending StartAllServices finished signal.");
                }
            }
            ServiceLifecycleCommand::StopService(ServiceSingleCommand { service_id, sender }) => {
                if let Err(error) = services.stop(&service_id).await {
                    error!(error=?error, "Error stopping service: {service_id:#?}");
                }
                if let Err(error) = sender.send(()) {
                    error!(error=?error, "Error sending StopService finished signal.");
                }
            }
            ServiceLifecycleCommand::StopServiceSequence(ServiceSequenceCommand {
                service_ids,
                sender,
            }) => {
                if let Err(error) = services.stop_sequence(service_ids.as_slice()).await {
                    error!(error=?error, "Error stopping services: {service_ids:#?}");
                }
                if let Err(error) = sender.send(()) {
                    error!(error=?error, "Error sending StopServiceSequence finished signal.");
                }
            }
            ServiceLifecycleCommand::StopAllServices(ServiceAllCommand { sender }) => {
                if let Err(error) = services.stop_all().await {
                    error!(error=?error, "Error stopping all services.");
                }
                if let Err(error) = sender.send(()) {
                    error!(error=?error, "Error sending StopAllServices finished signal.");
                }
            }
        }
    }
}
