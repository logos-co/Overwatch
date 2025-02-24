pub mod commands;
pub mod handle;
pub mod life_cycle;

// std
use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
// crates
use thiserror::Error;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
#[cfg(feature = "instrumentation")]
use tracing::instrument;
use tracing::{error, info};
// internal
use crate::overwatch::commands::{
    OverwatchCommand, OverwatchLifeCycleCommand, RelayCommand, ServiceLifeCycleCommand,
    SettingsCommand, StatusCommand,
};
use crate::overwatch::handle::OverwatchHandle;
pub use crate::overwatch::life_cycle::ServicesLifeCycleHandle;
use crate::services::life_cycle::LifecycleMessage;
use crate::services::relay::RelayResult;
use crate::services::status::ServiceStatusResult;
use crate::services::{ServiceError, ServiceId};
use crate::utils::runtime::default_multithread_runtime;

/// Overwatch base error type
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

/// Signal sent so overwatch finish execution
type FinishOverwatchSignal = ();

/// Marker trait for settings related elements
pub type AnySettings = Box<dyn Any + Send>;

/// An overwatch run anything that implements this trait
/// An implementor of this trait would have to handle the inner [`ServiceCore`](crate::services::ServiceCore)
pub trait Services: Sized {
    /// Inner [`ServiceCore::Settings`](crate::services::ServiceCore) grouping type.
    /// Normally this will be a settings object that group all the inner services settings.
    type Settings;

    /// Spawn a new instance of the Services object
    /// It returns a `(ServiceId, Runtime)` where Runtime is the `tokio::runtime::Runtime` attached for each
    /// service.
    /// It also returns an instance of the implementing type.
    fn new(
        settings: Self::Settings,
        overwatch_handle: OverwatchHandle,
    ) -> std::result::Result<Self, super::DynError>;

    /// Start a services attached to the trait implementer
    fn start(&mut self, service_id: ServiceId) -> Result<(), Error>;

    // TODO: this probably will be removed once the services lifecycle is implemented
    /// Start all services attached to the trait implementer
    fn start_all(&mut self) -> Result<ServicesLifeCycleHandle, Error>;

    /// Stop a service attached to the trait implementer
    fn stop(&mut self, service_id: ServiceId) -> Result<(), Error>;

    /// Request communication relay to one of the services
    fn request_relay(&mut self, service_id: ServiceId) -> RelayResult;

    fn request_status_watcher(&self, service_id: ServiceId) -> ServiceStatusResult;

    /// Update service settings
    fn update_settings(&mut self, settings: Self::Settings) -> Result<(), Error>;
}

/// `OverwatchRunner` is the entity that handles a running overwatch
/// it is usually one-shot. It contains what it is needed just to be run as a main loop
/// and a system to be able to stop it running. Meaning that it i responsible of the Overwatch
/// application lifecycle.
pub struct OverwatchRunner<Services> {
    services: Services,
    #[expect(unused)]
    handle: OverwatchHandle,
    finish_signal_sender: oneshot::Sender<()>,
    commands_receiver: Receiver<OverwatchCommand>,
}

/// Overwatch thread identifier
/// it is used when creating the `tokio::runtime::Runtime` that Overwatch uses internally
pub const OVERWATCH_THREAD_NAME: &str = "Overwatch";

impl<ServicesImpl> OverwatchRunner<ServicesImpl>
where
    ServicesImpl: Services + Send + 'static,
{
    /// Start the Overwatch runner process
    /// It creates the `tokio::runtime::Runtime`, initialize the [`Services`] and start listening for
    /// Overwatch related tasks.
    /// Returns the [`Overwatch`] instance that handles this runner.
    pub fn run(
        settings: ServicesImpl::Settings,
        runtime: Option<Runtime>,
    ) -> std::result::Result<Overwatch, super::DynError> {
        let runtime = runtime.unwrap_or_else(default_multithread_runtime);

        let (finish_signal_sender, finish_runner_signal) = tokio::sync::oneshot::channel();
        let (commands_sender, commands_receiver) = tokio::sync::mpsc::channel(16);
        let handle = OverwatchHandle::new(runtime.handle().clone(), commands_sender);
        let services = ServicesImpl::new(settings, handle.clone())?;
        let runner = OverwatchRunner {
            services,
            handle: handle.clone(),
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
                    Self::handle_status(&mut services, status_command);
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
        // signal that we finished execution
        finish_signal_sender
            .send(())
            .expect("Overwatch run finish signal to be sent properly");
    }

    fn handle_relay(services: &mut ServicesImpl, command: RelayCommand) {
        let RelayCommand {
            service_id,
            reply_channel,
        } = command;
        // send requested rely channel result to requesting service
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
        services: &mut ServicesImpl,
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

/// Main Overwatch entity
/// It manages the overwatch runtime and handle
pub struct Overwatch {
    runtime: Runtime,
    handle: OverwatchHandle,
    finish_runner_signal: oneshot::Receiver<FinishOverwatchSignal>,
}

impl Overwatch {
    /// Get the overwatch handle
    /// [`OverwatchHandle`] is cloneable, so it can be done on demand
    pub fn handle(&self) -> &OverwatchHandle {
        &self.handle
    }

    /// Get the underlaying tokio runtime handle
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

    /// Block until Overwatch finish its execution
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
    use crate::overwatch::handle::OverwatchHandle;
    use crate::overwatch::{Error, OverwatchRunner, Services, ServicesLifeCycleHandle};
    use crate::services::relay::{RelayError, RelayResult};
    use crate::services::status::{ServiceStatusError, ServiceStatusResult};
    use crate::services::ServiceId;
    use std::time::Duration;
    use tokio::time::sleep;

    struct EmptyServices;

    impl Services for EmptyServices {
        type Settings = ();

        fn new(
            _settings: Self::Settings,
            _overwatch_handle: OverwatchHandle,
        ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
            Ok(EmptyServices)
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
