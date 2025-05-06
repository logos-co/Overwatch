pub mod commands;
pub mod handle;

use std::{any::Any, fmt::Debug, future::Future};

use async_trait::async_trait;
use thiserror::Error;
use tokio::{
    runtime::{Handle, Runtime},
    sync::{mpsc::Receiver, oneshot},
    task::JoinHandle,
};
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
    },
    services::{
        life_cycle::{LifecycleHandle, LifecycleMessage},
        relay::RelayResult,
        status::StatusWatcher,
    },
    utils::runtime::default_multithread_runtime,
};

/// Overwatch base error type.
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Any(super::DynError),
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
#[async_trait]
pub trait Services: Sized {
    /// Inner [`ServiceCore::Settings`](crate::services::ServiceCore) grouping
    /// type.
    ///
    /// Normally this will be a settings object that groups all the inner
    /// services settings.
    type Settings;

    /// The type aggregating all different services identifiers that are part of
    /// this runtime implementation.
    ///
    /// This type is used by the services themselves to communicate with each
    /// other and to verify whether two services are part of the same runtime.
    type RuntimeServiceId;

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
        overwatch_handle: OverwatchHandle<Self::RuntimeServiceId>,
    ) -> Result<Self, super::DynError>;

    /// Start a service attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    async fn start(&mut self, service_id: &Self::RuntimeServiceId) -> Result<(), Error>;

    /// Start all services attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    async fn start_all(&mut self) -> Result<(), Error>;

    /// Stop a service attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    async fn stop(&mut self, service_id: &Self::RuntimeServiceId) -> Result<(), Error>;

    /// Stop all services attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    async fn stop_all(&mut self) -> Result<(), Error>;

    /// Request a communication relay for a service attached to the trait
    /// implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    /// TODO: No result
    fn request_relay(&mut self, service_id: &Self::RuntimeServiceId) -> RelayResult;

    /// Request a status watcher for a service attached to the trait
    /// implementer.
    fn request_status_watcher(&self, service_id: &Self::RuntimeServiceId) -> StatusWatcher;

    /// Update service settings for all services attached to the trait
    /// implementer.
    fn update_settings(&mut self, settings: Self::Settings);

    /// Get the [`LifecycleHandle`] for a service attached to the trait
    /// implementer.
    fn get_service_lifecycle_handle(&self, service_id: &Self::RuntimeServiceId)
        -> &LifecycleHandle;
}

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
    finish_signal_sender: oneshot::Sender<()>,
    commands_receiver: Receiver<OverwatchCommand<RuntimeServiceId>>,
}

pub type OverwatchRunner<ServicesImpl> =
    GenericOverwatchRunner<ServicesImpl, <ServicesImpl as Services>::RuntimeServiceId>;

/// Overwatch thread identifier.
///
/// It's used for creating the [`Runtime`] that Overwatch uses internally.
pub const OVERWATCH_THREAD_NAME: &str = "Overwatch";

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
    ) -> Result<Overwatch<ServicesImpl::RuntimeServiceId>, super::DynError> {
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
                OverwatchCommand::ServiceLifeCycle(msg) => match msg {
                    ServiceLifeCycleCommand {
                        service_id,
                        msg: shutdown_msg @ LifecycleMessage::Shutdown(_),
                    } => {
                        if let Err(e) = services
                            .get_service_lifecycle_handle(&service_id)
                            .send(shutdown_msg)
                        {
                            error!(e);
                        }
                    }
                    ServiceLifeCycleCommand {
                        service_id,
                        msg: start_msg @ LifecycleMessage::Start(_),
                    } => {
                        if let Err(e) = services
                            .get_service_lifecycle_handle(&service_id)
                            .send(start_msg)
                        {
                            error!(e);
                        }
                    }
                },
                OverwatchCommand::OverwatchLifeCycle(command) => match command {
                    OverwatchLifeCycleCommand::Start => {
                        if let Err(e) = services.start_all().await {
                            error!(error=?e, "Error starting all services");
                        }
                    }
                    OverwatchLifeCycleCommand::Shutdown | OverwatchLifeCycleCommand::Kill => {
                        if let Err(e) = services.stop_all().await {
                            error!(error=?e, "Error stopping all services");
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
        if let Err(Err(e)) = reply_channel.reply(services.request_relay(&service_id)) {
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

/// Main Overwatch entity.
/// It manages the [`Runtime`] and [`OverwatchHandle`].
pub struct Overwatch<RuntimeServiceId> {
    runtime: Runtime,
    handle: OverwatchHandle<RuntimeServiceId>,
    finish_runner_signal: oneshot::Receiver<FinishOverwatchSignal>,
}

impl<RuntimeServiceId> Overwatch<RuntimeServiceId> {
    /// Get the [`OverwatchHandle`]
    ///
    /// It's cloneable, so it can be done on demand
    pub const fn handle(&self) -> &OverwatchHandle<RuntimeServiceId> {
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

    use super::*;
    use crate::{
        overwatch::{handle::OverwatchHandle, Error, OverwatchRunner, Services},
        services::{life_cycle::LifecycleHandle, relay::RelayResult, status::StatusWatcher},
    };

    struct EmptyServices;

    #[async_trait]
    impl Services for EmptyServices {
        type Settings = ();
        type RuntimeServiceId = String;

        fn new(
            _settings: Self::Settings,
            _overwatch_handle: OverwatchHandle<String>,
        ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
            Ok(Self)
        }

        async fn start(&mut self, _service_id: &String) -> Result<(), Error> {
            Ok(())
        }

        async fn start_all(&mut self) -> Result<(), Error> {
            Ok(())
        }

        async fn stop(&mut self, _service_id: &String) -> Result<(), Error> {
            Ok(())
        }

        async fn stop_all(&mut self) -> Result<(), Error> {
            Ok(())
        }

        fn request_relay(&mut self, _service_id: &String) -> RelayResult {
            Ok(Box::new(()))
        }

        fn request_status_watcher(&self, _service_id: &String) -> StatusWatcher {
            unimplemented!("Not necessary for these tests.")
        }

        fn update_settings(&mut self, _settings: Self::Settings) {}

        fn get_service_lifecycle_handle(&self, _service_id: &String) -> &LifecycleHandle {
            unimplemented!("Not necessary for these tests.")
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
