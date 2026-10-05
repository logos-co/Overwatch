//! A panic in a `Service`'s `run` is caught by its `ServiceRunner`: the
//! `Service` ends up as `ServiceStatus::Failed` and, depending on the
//! `ServicePanicPolicy`, Overwatch shuts down.

use std::time::Duration;

use async_trait::async_trait;
use overwatch::{
    DynError, OpaqueServiceResourcesHandle,
    overwatch::{Overwatch, OverwatchRunner, ServicePanic, ServicePanicPolicy},
    services::{
        AsServiceId, ServiceCore, ServiceData,
        state::{NoOperator, NoState},
        status::{ServiceStatus, StatusWatcher},
    },
};
use overwatch_derive::derive_services;
use tokio::time::timeout;

const TIMEOUT: Duration = Duration::from_secs(5);
const PANIC_MESSAGE: &str = "FaultyService panicked on purpose";

#[derive(Clone, Copy, Debug)]
pub enum Fault {
    Panic,
    Error,
}

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

#[derive_services]
struct App {
    faulty_service: FaultyService,
    idle_service: IdleService,
}

struct Running {
    app: Overwatch<RuntimeServiceId>,
    faulty_status: StatusWatcher,
    idle_status: StatusWatcher,
}

async fn start(fault: Fault, policy: Option<ServicePanicPolicy>) -> Running {
    let settings = AppServiceSettings {
        faulty_service: fault,
        idle_service: (),
    };
    let runtime_handle = Some(tokio::runtime::Handle::current());
    let app = match policy {
        Some(policy) => {
            OverwatchRunner::<App>::run_with_panic_policy(settings, runtime_handle, policy)
        }
        None => OverwatchRunner::<App>::run(settings, runtime_handle),
    }
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

    Running {
        app,
        faulty_status,
        idle_status,
    }
}

#[tokio::test]
async fn panic_shuts_overwatch_down_by_default() {
    let Running {
        app,
        faulty_status,
        idle_status,
    } = start(Fault::Panic, None).await;

    let exit = timeout(TIMEOUT, app.wait_finished())
        .await
        .expect("Overwatch should finish after a service panic.");

    assert_eq!(
        exit,
        Err(ServicePanic {
            service_id: <RuntimeServiceId as AsServiceId<FaultyService>>::SERVICE_ID,
            message: PANIC_MESSAGE.to_owned(),
        })
    );
    assert_eq!(faulty_status.current(), ServiceStatus::Failed);
    assert_eq!(idle_status.current(), ServiceStatus::Stopped);
}

#[tokio::test]
async fn panic_stops_only_the_service_with_stop_service_policy() {
    let Running {
        app,
        mut faulty_status,
        idle_status,
    } = start(Fault::Panic, Some(ServicePanicPolicy::StopService)).await;

    faulty_status
        .wait_for(ServiceStatus::Failed, Some(TIMEOUT))
        .await
        .expect("The panicked service should be marked as failed.");

    // Overwatch still answers, which also means the failed service was cleaned
    // up: a Stop is only acknowledged once that is done.
    timeout(TIMEOUT, app.handle().stop_service::<FaultyService>())
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

#[tokio::test]
async fn failed_service_can_be_restarted() {
    let Running {
        app,
        mut faulty_status,
        ..
    } = start(Fault::Panic, Some(ServicePanicPolicy::StopService)).await;

    faulty_status
        .wait_for(ServiceStatus::Failed, Some(TIMEOUT))
        .await
        .expect("The panicked service should be marked as failed.");
    // Wait for the cleanup, otherwise the Start below is ignored.
    app.handle()
        .stop_service::<FaultyService>()
        .await
        .expect("Stopping a failed service should succeed.");

    app.handle()
        .start_service::<FaultyService>()
        .await
        .expect("A failed service should start again.");
    faulty_status
        .wait_for(ServiceStatus::Ready, Some(TIMEOUT))
        .await
        .expect("A restarted service should leave the failed status.");

    let _ = app.handle().shutdown().await;
    let exit = timeout(TIMEOUT, app.wait_finished())
        .await
        .expect("Overwatch should finish after a shutdown.");
    assert_eq!(exit, Ok(()));
}

#[tokio::test]
async fn waiting_for_another_status_returns_early_on_failure() {
    let Running {
        app,
        mut faulty_status,
        ..
    } = start(Fault::Panic, Some(ServicePanicPolicy::StopService)).await;

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
    let Running {
        app,
        mut faulty_status,
        idle_status,
    } = start(Fault::Error, None).await;

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
