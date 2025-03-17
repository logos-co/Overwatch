pub mod commands;
pub mod handle;
pub mod life_cycle;

use std::{any::Any, fmt::Debug, future::Future};

use thiserror::Error;
use tokio::{
    runtime::{Handle, Runtime},
    sync::{mpsc::Receiver, oneshot},
    task::JoinHandle,
};
#[cfg(feature = "instrumentation")]
use tracing::instrument;
use tracing::{error, info};

pub use crate::overwatch::life_cycle::ServicesLifeCycleHandle;
use crate::{
    overwatch::{
        commands::{
            OverwatchCommand, OverwatchLifeCycleCommand, RelayCommand, ServiceLifeCycleCommand,
            SettingsCommand, StatusCommand,
        },
        handle::OverwatchHandle,
    },
    services::{
        life_cycle::LifecycleMessage, relay::RelayResult, status::ServiceStatusResult,
        ServiceError, ServiceId,
    },
    utils::runtime::default_multithread_runtime,
};

/// Overwatch base error type.
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Relay(#[from] ServiceError),

    #[error("Service {service_id} is unavailable")]
    Unavailable { service_id: ServiceId },

    #[error(transparent)]
    Any(super::DynError),
}

impl Error {
    pub fn any<T: std::error::Error + Send + Sync + 'static>(err: T) -> Self {
        Self::Any(Box::new(err))
    }
}

impl From<super::DynError> for Error {
    fn from(err: super::DynError) -> Self {
        Self::Any(err)
    }
}

/// Signal sent when overwatch finishes execution.
type FinishOverwatchSignal = ();

/// Marker trait for settings' related elements.
pub type AnySettings = Box<dyn Any + Send>;

/// An Overwatch may run anything that implements this trait.
///
/// An implementor of this trait would have to handle the inner.
/// [`ServiceCore`](crate::services::ServiceCore).
pub trait Services: Sized {
    /// Inner [`ServiceCore::Settings`](crate::services::ServiceCore) grouping
    /// type.
    ///
    /// Normally this will be a settings object that groups all the inner
    /// services settings.
    type Settings;

    /// Spawn a new instance of the [`Services`] object.
    ///
    /// It returns a `(ServiceId, Runtime)` where Runtime is the [`Runtime`]
    /// attached for each service.
    ///
    /// It also returns an instance of the implementing type.
    ///
    /// # Errors
    ///
    /// The implementer's creation error.
    fn new(
        settings: Self::Settings,
        overwatch_handle: OverwatchHandle,
    ) -> Result<Self, super::DynError>;

    /// Start a service attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    fn start(&mut self, service_id: ServiceId) -> Result<(), Error>;

    // TODO: this probably will be removed once the services lifecycle is
    // implemented
    /// Start all services attaches to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    fn start_all(&mut self) -> Result<ServicesLifeCycleHandle, Error>;

    /// Stop a service attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    fn stop(&mut self, service_id: ServiceId) -> Result<(), Error>;

    /// Request a communication relay for a service.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    fn request_relay(&mut self, service_id: ServiceId) -> RelayResult;

    /// Request a status watcher for a service.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    fn request_status_watcher(&self, service_id: ServiceId) -> ServiceStatusResult;

    /// Update service settings.
    /// # Errors
    ///
    /// The generated [`Error`].
    fn update_settings(&mut self, settings: Self::Settings) -> Result<(), Error>;
}

/// Handle a running [`Overwatch`].
///
/// It's usually one-shot.
///
/// It only contains what's required to run [`Overwatch`] as a main loop and to
/// be able to stop it.
///
/// That is, it's responsible for [`Overwatch`]'s application lifecycle.
pub struct OverwatchRunner<Services> {
    services: Services,
    finish_signal_sender: oneshot::Sender<()>,
    commands_receiver: Receiver<OverwatchCommand>,
}

/// Overwatch thread identifier.
///
/// It's used for creating the [`Runtime`] that Overwatch uses internally.
pub const OVERWATCH_THREAD_NAME: &str = "Overwatch";

impl<ServicesImpl> OverwatchRunner<ServicesImpl>
where
    ServicesImpl: Services + Send + 'static,
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
    ) -> Result<Overwatch, super::DynError> {
        let runtime = runtime.unwrap_or_else(default_multithread_runtime);

        let (finish_signal_sender, finish_runner_signal) = oneshot::channel();
        let (commands_sender, commands_receiver) = tokio::sync::mpsc::channel(16);
        let handle = OverwatchHandle::new(runtime.handle().clone(), commands_sender);
        let services = ServicesImpl::new(settings, handle.clone())?;
        let runner = Self {
            services,
            finish_signal_sender,
            commands_receiver,
        };

        runtime.spawn(async move { runner.run_().await });

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
            ..
        } = self;
        let lifecycle_handlers = services.start_all().expect("Services to start running");
        while let Some(command) = commands_receiver.recv().await {
            info!(command = ?command, "Overwatch command received");
            match command {
                OverwatchCommand::Relay(relay_command) => {
                    Self::handle_relay(&mut services, relay_command);
                }
                OverwatchCommand::Status(status_command) => {
                    Self::handle_status(&services, status_command);
                }
                OverwatchCommand::ServiceLifeCycle(msg) => match msg {
                    ServiceLifeCycleCommand {
                        service_id,
                        msg: LifecycleMessage::Shutdown(channel),
                    } => {
                        if let Err(e) = lifecycle_handlers.shutdown(service_id, channel) {
                            error!(e);
                        }
                    }
                    ServiceLifeCycleCommand {
                        service_id,
                        msg: LifecycleMessage::Kill,
                    } => {
                        if let Err(e) = lifecycle_handlers.kill(service_id) {
                            error!(e);
                        }
                    }
                },
                OverwatchCommand::OverwatchLifeCycle(command) => {
                    if matches!(
                        command,
                        OverwatchLifeCycleCommand::Kill | OverwatchLifeCycleCommand::Shutdown
                    ) {
                        if let Err(e) = lifecycle_handlers.kill_all() {
                            error!(e);
                        }
                        break;
                    }
                }
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

    fn handle_relay(services: &mut ServicesImpl, command: RelayCommand) {
        let RelayCommand {
            service_id,
            reply_channel,
        } = command;
        // Send the requested reply channel result to the requesting service
        if let Err(Err(e)) = reply_channel.reply(services.request_relay(service_id)) {
            info!(error=?e, "Error requesting relay for service {service_id}");
        }
    }

    fn handle_settings_update(services: &mut ServicesImpl, command: SettingsCommand) {
        let SettingsCommand(settings) = command;
        if let Ok(settings) = settings.downcast::<ServicesImpl::Settings>() {
            if let Err(e) = services.update_settings(*settings) {
                // TODO: add proper logging
                error!("{e}");
            }
        } else {
            unreachable!("Statically should always be of the correct type");
        }
    }

    fn handle_status(
        services: &ServicesImpl,
        StatusCommand {
            service_id,
            reply_channel,
        }: StatusCommand,
    ) {
        let watcher_result = services.request_status_watcher(service_id);
        match watcher_result {
            Ok(watcher) => {
                if reply_channel.reply(watcher).is_err() {
                    error!("Error reporting back status watcher for service: {service_id}");
                }
            }
            Err(e) => {
                error!("{e}");
            }
        }
    }
}

/// Main Overwatch entity.
/// It manages the [`Runtime`] and [`OverwatchHandle`].
pub struct Overwatch {
    runtime: Runtime,
    handle: OverwatchHandle,
    finish_runner_signal: oneshot::Receiver<FinishOverwatchSignal>,
}

impl Overwatch {
    /// Get the [`OverwatchHandle`]
    ///
    /// It's cloneable, so it can be done on demand
    pub const fn handle(&self) -> &OverwatchHandle {
        &self.handle
    }

    /// Get the underlying [`Handle`]
    pub fn runtime(&self) -> &Handle {
        self.runtime.handle()
    }

    /// Spawn a new task within the Overwatch runtime
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(future)
    }

    /// Block until Overwatch finishes executing.
    ///
    /// # Panics
    ///
    /// If the termination signal is never received.
    pub fn wait_finished(self) {
        let Self {
            runtime,
            finish_runner_signal,
            ..
        } = self;
        runtime.block_on(async move {
            let signal_result = finish_runner_signal.await;
            signal_result.expect("A finished signal arrived");
        });
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use tokio::time::sleep;

    use crate::{
        overwatch::{
            handle::OverwatchHandle, Error, OverwatchRunner, Services, ServicesLifeCycleHandle,
        },
        services::{
            relay::{RelayError, RelayResult},
            status::{ServiceStatusError, ServiceStatusResult},
            ServiceId,
        },
    };

    struct EmptyServices;

    impl Services for EmptyServices {
        type Settings = ();

        fn new(
            _settings: Self::Settings,
            _overwatch_handle: OverwatchHandle,
        ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
            Ok(Self)
        }

        fn start(&mut self, service_id: ServiceId) -> Result<(), Error> {
            Err(Error::Unavailable { service_id })
        }

        fn start_all(&mut self) -> Result<ServicesLifeCycleHandle, Error> {
            Ok(ServicesLifeCycleHandle::empty())
        }

        fn stop(&mut self, service_id: ServiceId) -> Result<(), Error> {
            Err(Error::Unavailable { service_id })
        }

        fn request_relay(&mut self, service_id: ServiceId) -> RelayResult {
            Err(RelayError::InvalidRequest { to: service_id })
        }

        fn request_status_watcher(&self, service_id: ServiceId) -> ServiceStatusResult {
            Err(ServiceStatusError::Unavailable { service_id })
        }

        fn update_settings(&mut self, _settings: Self::Settings) -> Result<(), Error> {
            Ok(())
        }
    }

    #[test]
    fn run_overwatch_then_stop() {
        let overwatch = OverwatchRunner::<EmptyServices>::run((), None).unwrap();
        let handle = overwatch.handle().clone();

        overwatch.spawn(async move {
            sleep(Duration::from_millis(500)).await;
            handle.shutdown().await;
        });

        overwatch.wait_finished();
    }

    #[test]
    fn run_overwatch_then_kill() {
        let overwatch = OverwatchRunner::<EmptyServices>::run((), None).unwrap();
        let handle = overwatch.handle().clone();

        overwatch.spawn(async move {
            sleep(Duration::from_millis(500)).await;
            handle.kill().await;
        });

        overwatch.wait_finished();
    }
}
