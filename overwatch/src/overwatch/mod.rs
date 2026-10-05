pub mod commands;
pub mod errors;
pub mod handle;
pub mod panic;
pub mod runner;
mod runtime;
pub mod services;

use std::any::Any;

pub use errors::{DynError, Error};
pub use handle::OverwatchHandle;
pub use panic::{ExitResult, PanicPolicy, ServicePanic, Shutdown};
pub use runner::{GenericOverwatchRunner, OVERWATCH_THREAD_NAME, OverwatchRunner};
pub use services::Services;
use tokio::{sync::oneshot, task::JoinHandle};

use crate::overwatch::runtime::OverwatchRuntime;

/// Marker trait for settings' related elements.
pub type AnySettings = Box<dyn Any + Send>;

/// Main Overwatch entity.
/// It manages the `OverwatchRuntime` and [`OverwatchHandle`].
pub struct Overwatch<RuntimeServiceId> {
    runtime: OverwatchRuntime,
    handle: OverwatchHandle<RuntimeServiceId>,
    finish_runner_signal: oneshot::Receiver<ExitResult<RuntimeServiceId>>,
}

impl<RuntimeServiceId> Overwatch<RuntimeServiceId> {
    /// Get the [`OverwatchHandle`]
    ///
    /// It's cloneable, so it can be done on demand
    pub const fn handle(&self) -> &OverwatchHandle<RuntimeServiceId> {
        &self.handle
    }

    /// Get the underlying `OverwatchRuntime`
    pub const fn runtime(&self) -> &OverwatchRuntime {
        &self.runtime
    }

    /// Spawn a new task within the Overwatch runtime
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.handle().spawn(future)
    }

    /// Wait until [`Overwatch`] finishes executing.
    ///
    /// # Errors
    ///
    /// If [`Overwatch`] was shut down because a `Service` panicked. See
    /// [`OverwatchHandle::shutdown_with_panic`].
    ///
    /// # Panics
    ///
    /// If the termination signal is never received.
    pub async fn wait_finished(self) -> ExitResult<RuntimeServiceId> {
        let Self {
            finish_runner_signal,
            ..
        } = self;

        handle_finish_signal(finish_runner_signal).await
    }

    /// Block until [`Overwatch`] finishes executing.
    ///
    /// # Errors
    ///
    /// If [`Overwatch`] was shut down because a `Service` panicked. See
    /// [`OverwatchHandle::shutdown_with_panic`].
    ///
    /// # Panics
    ///
    /// If the termination signal is never received.
    pub fn blocking_wait_finished(self) -> ExitResult<RuntimeServiceId> {
        let Self {
            runtime,
            finish_runner_signal,
            ..
        } = self;

        runtime
            .handle()
            .block_on(handle_finish_signal(finish_runner_signal))
    }
}

/// Handle the finish signal for [`Overwatch`]
async fn handle_finish_signal<RuntimeServiceId>(
    finish_runner_signal: oneshot::Receiver<ExitResult<RuntimeServiceId>>,
) -> ExitResult<RuntimeServiceId> {
    let signal_result = finish_runner_signal.await;
    signal_result.expect("A finished signal arrived")
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio::time::sleep;

    use crate::{
        overwatch::{Error, OverwatchRunner, Services, Shutdown, handle::OverwatchHandle},
        services::{lifecycle::LifecycleNotifier, relay::AnyMessage, status::StatusWatcher},
    };

    struct EmptyServices;

    #[async_trait]
    impl Services for EmptyServices {
        type Settings = ();
        type RuntimeServiceId = String;
        type PanicPolicy = Option<Shutdown>;

        fn new(
            _settings: Self::Settings,
            _overwatch_handle: OverwatchHandle<String>,
        ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
            Ok(Self)
        }

        async fn start(&mut self, _service_id: &String) -> Result<(), Error> {
            Ok(())
        }

        async fn start_sequence(
            &mut self,
            _service_ids: &[Self::RuntimeServiceId],
        ) -> Result<(), Error> {
            Ok(())
        }

        async fn start_all(&mut self) -> Result<(), Error> {
            Ok(())
        }

        async fn stop(&mut self, _service_id: &String) -> Result<(), Error> {
            Ok(())
        }

        async fn stop_sequence(
            &mut self,
            _service_ids: &[Self::RuntimeServiceId],
        ) -> Result<(), Error> {
            Ok(())
        }

        async fn stop_all(&mut self) -> Result<(), Error> {
            Ok(())
        }

        async fn teardown(self) -> Result<(), Error> {
            Ok(())
        }

        fn ids(&self) -> Vec<Self::RuntimeServiceId> {
            vec![]
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
            let _ = handle.shutdown().await;
        });

        overwatch.blocking_wait_finished().unwrap();
    }

    #[test]
    fn run_overwatch_then_shutdown() {
        let overwatch = OverwatchRunner::<EmptyServices>::run((), None).unwrap();
        let handle = overwatch.handle().clone();

        overwatch.spawn(async move {
            sleep(Duration::from_millis(500)).await;
            let _ = handle.shutdown().await;
        });

        overwatch.blocking_wait_finished().unwrap();
    }
}
