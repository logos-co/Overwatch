use std::fmt::Debug;

use tokio::{runtime::Runtime, sync::mpsc::Receiver};
#[cfg(feature = "instrumentation")]
use tracing::instrument;
use tracing::{error, info};

use crate::{
    overwatch::{
        commands::{
            OverwatchCommand, OverwatchLifeCycleCommand, RelayCommand, ServiceLifeCycleCommand,
            SettingsCommand, StatusCommand,
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
                    Self::handle_relay(&mut services, relay_command);
                }
                OverwatchCommand::Status(status_command) => {
                    Self::handle_status(&services, status_command);
                }
                OverwatchCommand::ServiceLifeCycle(msg) => {
                    let ServiceLifeCycleCommand {
                        service_id,
                        msg: lifecycle_msg,
                    } = msg;
                    let lifecycle_notifier = services.get_service_lifecycle_notifier(&service_id);
                    if let Err(e) = lifecycle_notifier.send(lifecycle_msg).await {
                        error!(e);
                    }
                }
                OverwatchCommand::OverwatchLifeCycle(command) => match command {
                    OverwatchLifeCycleCommand::StartAllServices => {
                        if let Err(e) = services.start_all().await {
                            error!(error=?e, "Error starting all services.");
                        }
                    }
                    OverwatchLifeCycleCommand::StopAllServices => {
                        if let Err(e) = services.stop_all().await {
                            error!(error=?e, "Error stopping all services.");
                        }
                    }
                    OverwatchLifeCycleCommand::Shutdown => {
                        if let Err(e) = services.stop_all().await {
                            error!(error=?e, "Error stopping all services during teardown.");
                        }
                        if let Err(e) = services.teardown().await {
                            error!(error=?e, "Error tearing down services.");
                        }
                        break;
                    }
                },
                OverwatchCommand::Settings(settings) => {
                    Self::handle_settings_update(&mut services, settings);
                }
            }
        }
        // Signal that we finished execution
        finish_signal_sender
            .send(())
            .expect("Overwatch run finish signal to be sent properly");
    }

    fn handle_relay(
        services: &mut ServicesImpl,
        command: RelayCommand<ServicesImpl::RuntimeServiceId>,
    ) {
        let RelayCommand {
            service_id,
            reply_channel,
        } = command;
        // Send the requested reply channel result to the requesting service
        if let Err(e) = reply_channel.reply(services.request_relay(&service_id)) {
            info!(error=?e, "Error requesting relay for service {service_id:#?}");
        }
    }

    fn handle_settings_update(services: &mut ServicesImpl, command: SettingsCommand) {
        let SettingsCommand(settings) = command;
        let Ok(settings) = settings.downcast::<ServicesImpl::Settings>() else {
            unreachable!("Statically should always be of the correct type");
        };
        services.update_settings(*settings);
    }

    fn handle_status(
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
}
