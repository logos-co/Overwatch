pub mod commands;
pub mod errors;
pub mod handle;
pub mod runner;
pub mod services;

use std::{any::Any, future::Future};

pub use errors::{DynError, Error};
pub use handle::OverwatchHandle;
pub use runner::{GenericOverwatchRunner, OverwatchRunner, OVERWATCH_THREAD_NAME};
pub use services::Services;
use tokio::{
    runtime::{Handle, Runtime},
    task::JoinHandle,
};

use crate::utils::finished_signal;

/// Marker trait for settings' related elements.
pub type AnySettings = Box<dyn Any + Send>;

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

    use async_trait::async_trait;
    use tokio::time::sleep;

    use crate::{
        overwatch::{handle::OverwatchHandle, Error, OverwatchRunner, Services},
        services::{lifecycle::LifecycleNotifier, relay::AnyMessage, status::StatusWatcher},
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
