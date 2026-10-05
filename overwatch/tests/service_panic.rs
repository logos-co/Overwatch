//! A panic in a `Service`'s `run` is caught by its `ServiceRunner`: the
//! `Service` ends up as `ServiceStatus::Failed` and the application's
//! `PanicPolicy` decides what happens next.

use std::{
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use overwatch::{
    DynError, OpaqueServiceResourcesHandle,
    overwatch::{
        NoPolicy, Overwatch, OverwatchHandle, OverwatchRunner, PanicPolicy, ServicePanic,
        ShutdownOverwatch,
    },
    services::{
        AsServiceId, ServiceCore, ServiceData,
        state::{NoOperator, NoState},
        status::{ServiceStatus, StatusWatcher},
    },
};
use overwatch_derive::derive_services;
use tokio::{sync::mpsc, time::timeout};

const TIMEOUT: Duration = Duration::from_secs(5);
const PANIC_MESSAGE: &str = "FaultyService panicked on purpose";

#[derive(Clone, Copy, Debug)]
pub enum Fault {
    Panic,
    Error,
}

/// Defines, in its own module, an application with a faulty and an idle
/// service that uses the given panic policy.
///
/// The policy belongs to the application, so each policy under test needs
/// its own.
macro_rules! app_with_panic_policy {
    ($module:ident, $panic_policy:ty) => {
        mod $module {
            use super::*;

            pub struct FaultyService {
                service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
            }

            impl ServiceData for FaultyService {
                type Settings = Fault;
                type State = NoState<Self::Settings>;
                type StateOperator = NoOperator<Self::State>;
                type Message = ();
            }

            #[async_trait]
            impl ServiceCore<RuntimeServiceId> for FaultyService {
                fn init(
                    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
                    _initial_state: Self::State,
                ) -> Result<Self, DynError> {
                    Ok(Self {
                        service_resources_handle,
                    })
                }

                async fn run(self) -> Result<(), DynError> {
                    self.service_resources_handle.status_updater.notify_ready();
                    let fault = self
                        .service_resources_handle
                        .settings_handle
                        .notifier()
                        .get_updated_settings();
                    // Let the other services start.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    match fault {
                        Fault::Panic => panic!("{PANIC_MESSAGE}"),
                        Fault::Error => Err("FaultyService failed on purpose".into()),
                    }
                }
            }

            pub struct IdleService {
                service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
            }

            impl ServiceData for IdleService {
                type Settings = ();
                type State = NoState<Self::Settings>;
                type StateOperator = NoOperator<Self::State>;
                type Message = ();
            }

            #[async_trait]
            impl ServiceCore<RuntimeServiceId> for IdleService {
                fn init(
                    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
                    _initial_state: Self::State,
                ) -> Result<Self, DynError> {
                    Ok(Self {
                        service_resources_handle,
                    })
                }

                async fn run(self) -> Result<(), DynError> {
                    self.service_resources_handle.status_updater.notify_ready();
                    std::future::pending::<()>().await;
                    Ok(())
                }
            }

            #[derive_services(panic_policy = $panic_policy)]
            struct App {
                faulty_service: FaultyService,
                idle_service: IdleService,
            }

            /// Starts the application.
            ///
            /// # Returns
            ///
            /// Overwatch and the status watchers of the faulty and the idle
            /// service, in that order.
            pub async fn start(
                fault: Fault,
            ) -> (Overwatch<RuntimeServiceId>, StatusWatcher, StatusWatcher) {
                let settings = AppServiceSettings {
                    faulty_service: fault,
                    idle_service: (),
                };
                let runtime_handle = Some(tokio::runtime::Handle::current());
                let app = OverwatchRunner::<App>::run(settings, runtime_handle)
                    .expect("OverwatchRunner should start.");

                let faulty_status = app
                    .handle()
                    .status_watcher::<FaultyService>()
                    .await
                    .expect("Status watcher should be available.");
                let idle_status = app
                    .handle()
                    .status_watcher::<IdleService>()
                    .await
                    .expect("Status watcher should be available.");

                app.handle()
                    .start_all_services()
                    .await
                    .expect("Services should start.");

                (app, faulty_status, idle_status)
            }
        }
    };
}

app_with_panic_policy!(shutdown_overwatch, ShutdownOverwatch);
app_with_panic_policy!(no_policy, NoPolicy);
app_with_panic_policy!(restart_once, RestartOnce);
app_with_panic_policy!(plain_shutdown, PlainShutdown);

#[tokio::test]
async fn shutdown_overwatch_policy_shuts_overwatch_down() {
    let (app, faulty_status, idle_status) = shutdown_overwatch::start(Fault::Panic).await;

    let exit = timeout(TIMEOUT, app.wait_finished())
        .await
        .expect("Overwatch should finish after a service panic.");

    assert_eq!(
        exit,
        Err(ServicePanic {
            service_id: <shutdown_overwatch::RuntimeServiceId as AsServiceId<
                shutdown_overwatch::FaultyService,
            >>::SERVICE_ID,
            message: PANIC_MESSAGE.to_owned(),
        })
    );
    assert_eq!(faulty_status.current(), ServiceStatus::Failed);
    assert_eq!(idle_status.current(), ServiceStatus::Stopped);
}

#[tokio::test]
async fn no_policy_stops_only_the_panicked_service() {
    let (app, mut faulty_status, idle_status) = no_policy::start(Fault::Panic).await;

    faulty_status
        .wait_for(ServiceStatus::Failed, Some(TIMEOUT))
        .await
        .expect("The panicked service should be marked as failed.");

    // Overwatch still answers, and a failed service handles a Stop as a stopped
    // one does.
    timeout(
        TIMEOUT,
        app.handle().stop_service::<no_policy::FaultyService>(),
    )
    .await
    .expect("Stopping a failed service should not hang.")
    .expect("Stopping a failed service should succeed.");
    assert_eq!(faulty_status.current(), ServiceStatus::Failed);
    assert_eq!(idle_status.current(), ServiceStatus::Ready);

    app.handle()
        .shutdown()
        .await
        .expect("Overwatch should shut down.");
    let exit = timeout(TIMEOUT, app.wait_finished())
        .await
        .expect("Overwatch should finish after a shutdown.");
    assert_eq!(exit, Ok(()));
}

type RestartOnceCall = (ServicePanic<restart_once::RuntimeServiceId>, ServiceStatus);

static RESTART_ONCE_RESTARTED: AtomicBool = AtomicBool::new(false);
static RESTART_ONCE_CALLS: OnceLock<mpsc::UnboundedSender<RestartOnceCall>> = OnceLock::new();

/// Reports every call, with the status the panicked service had at that point,
/// and restarts the service the first time.
///
/// A policy is built with `Default`, so its state lives in statics.
#[derive(Default)]
struct RestartOnce;

#[async_trait]
impl PanicPolicy<restart_once::RuntimeServiceId> for RestartOnce {
    async fn on_service_panic(
        &self,
        service_panic: ServicePanic<restart_once::RuntimeServiceId>,
        overwatch_handle: &OverwatchHandle<restart_once::RuntimeServiceId>,
    ) {
        let status = overwatch_handle
            .status_watcher::<restart_once::FaultyService>()
            .await
            .expect("Status watcher should be available.")
            .current();
        let calls = RESTART_ONCE_CALLS
            .get()
            .expect("The test should set the calls channel.");
        let _ = calls.send((service_panic, status));

        if !RESTART_ONCE_RESTARTED.swap(true, Ordering::SeqCst) {
            overwatch_handle
                .start_service::<restart_once::FaultyService>()
                .await
                .expect("A failed service should start again.");
        }
    }
}

#[tokio::test]
async fn custom_policy_can_restart_the_failed_service() {
    let (calls, mut calls_receiver) = mpsc::unbounded_channel();
    RESTART_ONCE_CALLS
        .set(calls)
        .expect("Only this test should set the calls channel.");

    let (app, ..) = restart_once::start(Fault::Panic).await;

    // The second call can only happen if the restart worked: the restarted
    // service panics again.
    for _ in 0..2 {
        let (service_panic, status) = timeout(TIMEOUT, calls_receiver.recv())
            .await
            .expect("The policy should be called for every panic.")
            .expect("The policy should be alive.");
        assert_eq!(
            service_panic,
            ServicePanic {
                service_id: <restart_once::RuntimeServiceId as AsServiceId<
                    restart_once::FaultyService,
                >>::SERVICE_ID,
                message: PANIC_MESSAGE.to_owned(),
            }
        );
        assert_eq!(status, ServiceStatus::Failed);
    }

    app.handle()
        .shutdown()
        .await
        .expect("Overwatch should shut down.");
    let exit = timeout(TIMEOUT, app.wait_finished())
        .await
        .expect("Overwatch should finish after a shutdown.");
    assert_eq!(exit, Ok(()));
}

/// Shuts Overwatch down without reporting the panic as the cause.
#[derive(Default)]
struct PlainShutdown;

#[async_trait]
impl PanicPolicy<plain_shutdown::RuntimeServiceId> for PlainShutdown {
    async fn on_service_panic(
        &self,
        _service_panic: ServicePanic<plain_shutdown::RuntimeServiceId>,
        overwatch_handle: &OverwatchHandle<plain_shutdown::RuntimeServiceId>,
    ) {
        overwatch_handle
            .shutdown()
            .await
            .expect("Overwatch should shut down.");
    }
}

#[tokio::test]
async fn custom_policy_can_shut_overwatch_down() {
    let (app, faulty_status, idle_status) = plain_shutdown::start(Fault::Panic).await;

    let exit = timeout(TIMEOUT, app.wait_finished())
        .await
        .expect("Overwatch should finish after the policy shuts it down.");

    assert_eq!(exit, Ok(()));
    assert_eq!(faulty_status.current(), ServiceStatus::Failed);
    assert_eq!(idle_status.current(), ServiceStatus::Stopped);
}

#[tokio::test]
async fn waiting_for_another_status_returns_early_on_failure() {
    let (app, mut faulty_status, _idle_status) = no_policy::start(Fault::Panic).await;

    let result = timeout(
        TIMEOUT,
        faulty_status.wait_for(ServiceStatus::Stopped, None),
    )
    .await
    .expect("Waiting on a failed service should not hang.");
    assert_eq!(result, Err(ServiceStatus::Failed));

    let _ = app.handle().shutdown().await;
    let exit = timeout(TIMEOUT, app.wait_finished())
        .await
        .expect("Overwatch should finish after a shutdown.");
    assert_eq!(exit, Ok(()));
}

#[tokio::test]
async fn error_return_stops_only_the_service() {
    let (app, mut faulty_status, idle_status) = shutdown_overwatch::start(Fault::Error).await;

    faulty_status
        .wait_for(ServiceStatus::Stopped, Some(TIMEOUT))
        .await
        .expect("A service returning an error should be stopped, not failed.");
    assert_eq!(idle_status.current(), ServiceStatus::Ready);

    app.handle()
        .shutdown()
        .await
        .expect("Overwatch should shut down.");
    let exit = timeout(TIMEOUT, app.wait_finished())
        .await
        .expect("Overwatch should finish after a shutdown.");
    assert_eq!(exit, Ok(()));
}
