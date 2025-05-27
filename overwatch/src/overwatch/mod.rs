pub mod commands;
pub mod errors;
pub mod handle;
pub mod runner;

use std::{any::Any, future::Future};

use async_trait::async_trait;
pub use errors::{DynError, Error};
pub use runner::{GenericOverwatchRunner, OverwatchRunner, OVERWATCH_THREAD_NAME};
use tokio::{
    runtime::{Handle, Runtime},
    task::JoinHandle,
};

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{lifecycle::LifecycleNotifier, relay::AnyMessage, status::StatusWatcher},
    utils::finished_signal,
};

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
    ) -> Result<Self, DynError>;

    /// Start a service attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn start(&mut self, service_id: &Self::RuntimeServiceId) -> Result<(), Error>;

    /// Start all services attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn start_all(&mut self) -> Result<(), Error>;

    /// Stop a service attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn stop(&mut self, service_id: &Self::RuntimeServiceId) -> Result<(), Error>;

    /// Stop all services attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn stop_all(&mut self) -> Result<(), Error>;

    /// Shuts down the `Service`'s
    /// [`ServiceRunner`](crate::services::runner::ServiceRunner)s attached to
    /// the trait implementer.
    ///
    /// Depending on the implementation, this may be a no-op.
    ///
    /// This is the opposite operation of [`Self::new`]: It's _final_.
    /// `Service`s won't be able to be started again after calling it.
    ///
    /// # Note
    ///
    /// The current implementation of this function (when derived via the
    /// [`#[derive_services]`](overwatch_derive::derive_services) macro)
    /// kills the [`ServiceRunner`](crate::services::runner::ServiceRunner)s
    /// without waiting for their respective `Service`s to finish.
    /// If you want to wait for the `Service`s to finish, you should call
    /// [`Self::stop_all`] first.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn teardown(self) -> Result<(), Error>;

    /// Request a communication relay for a service attached to the trait
    /// implementer.
    fn request_relay(&mut self, service_id: &Self::RuntimeServiceId) -> AnyMessage;

    /// Request a status watcher for a service attached to the trait
    /// implementer.
    fn request_status_watcher(&self, service_id: &Self::RuntimeServiceId) -> StatusWatcher;

    /// Update service settings for all services attached to the trait
    /// implementer.
    fn update_settings(&mut self, settings: Self::Settings);

    /// Get the [`LifecycleNotifier`] for a service attached to the trait
    /// implementer.
    fn get_service_lifecycle_notifier(
        &self,
        service_id: &Self::RuntimeServiceId,
    ) -> &LifecycleNotifier;
}

/// Main Overwatch entity.
/// It manages the [`Runtime`] and [`OverwatchHandle`].
pub struct Overwatch<RuntimeServiceId> {
    runtime: Runtime,
    handle: OverwatchHandle<RuntimeServiceId>,
    finish_runner_signal: finished_signal::Receiver,
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
        services::{lifecycle::LifecycleNotifier, status::StatusWatcher},
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

        async fn teardown(self) -> Result<(), Error> {
            Ok(())
        }

        fn request_relay(&mut self, _service_id: &String) -> AnyMessage {
            Box::new(())
        }

        fn request_status_watcher(&self, _service_id: &String) -> StatusWatcher {
            unimplemented!("Not necessary for these tests.")
        }

        fn update_settings(&mut self, _settings: Self::Settings) {}

        fn get_service_lifecycle_notifier(&self, _service_id: &String) -> &LifecycleNotifier {
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
    fn run_overwatch_then_shutdown() {
        let overwatch = OverwatchRunner::<EmptyServices>::run((), None).unwrap();
        let handle = overwatch.handle().clone();

        overwatch.spawn(async move {
            sleep(Duration::from_millis(500)).await;
            handle.shutdown().await;
        });

        overwatch.wait_finished();
    }
}
