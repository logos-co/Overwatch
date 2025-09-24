use async_trait::async_trait;
use overwatch::{
    DynError, OpaqueServiceResourcesHandle,
    overwatch::{Overwatch, OverwatchRunner},
    services::{
        AsServiceId, ServiceCore, ServiceData,
        state::{NoOperator, ServiceState},
        status::{ServiceStatus, StatusWatcher},
    },
};
use overwatch_derive::derive_services;
use tokio::runtime::Handle;

#[derive(Clone)]
struct ServiceStateA;

impl ServiceState for ServiceStateA {
    type Settings = ();
    type Error = DynError;

    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

#[derive(Clone)]
struct ServiceStateB;

impl ServiceState for ServiceStateB {
    type Settings = ();
    type Error = DynError;

    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

struct ServiceA {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

impl ServiceData for ServiceA {
    type Settings = ();
    type State = ServiceStateA;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for ServiceA {
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

        // Simulate some work so Status::Ready can be observed
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

struct ServiceB {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

impl ServiceData for ServiceB {
    type Settings = ();
    type State = ServiceStateB;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for ServiceB {
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

        // Simulate some work so Status::Stopped can be observed
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

#[derive(Clone)]
struct ServiceStateC;

impl ServiceState for ServiceStateC {
    type Settings = ();
    type Error = DynError;

    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

struct ServiceC {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

impl ServiceData for ServiceC {
    type Settings = ();
    type State = ServiceStateC;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for ServiceC {
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

        // Simulate some work so Status::Ready can be observed
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

#[derive_services]
struct App {
    service_a: ServiceA,
    service_b: ServiceB,
    service_c: ServiceC,
}

fn initialize() -> Overwatch<RuntimeServiceId> {
    let settings = AppServiceSettings {
        service_a: (),
        service_b: (),
        service_c: (),
    };
    OverwatchRunner::<App>::run(settings, None).expect("Failed to run overwatch")
}

fn wait_for_status(
    handle: &Handle,
    status_watcher: &mut StatusWatcher,
    expected_status: ServiceStatus,
) {
    handle
        .block_on(
            status_watcher
                .receiver_mut()
                .wait_for(|status| *status == expected_status),
        )
        .expect("Failed to wait for status");
}

#[test]
fn test_start() {
    let overwatch = initialize();
    overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().start_service::<ServiceA>())
        .expect("Failed to start service A");

    let status_watcher_a = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceA>());
    let status_watcher_b = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceB>());
    let status_watcher_c = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceC>());

    assert_eq!(status_watcher_a.current(), ServiceStatus::Ready);
    assert_eq!(status_watcher_b.current(), ServiceStatus::Stopped);
    assert_eq!(status_watcher_c.current(), ServiceStatus::Stopped);

    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().shutdown());
}

#[test]
fn test_start_list() {
    let overwatch = initialize();
    let services: Vec<RuntimeServiceId> = vec![
        <RuntimeServiceId as AsServiceId<ServiceA>>::SERVICE_ID,
        <RuntimeServiceId as AsServiceId<ServiceB>>::SERVICE_ID,
    ];

    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().start_service_sequence(services));

    let status_watcher_a = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceA>());
    let status_watcher_b = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceB>());
    let status_watcher_c = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceC>());

    assert_eq!(status_watcher_a.current(), ServiceStatus::Ready);
    assert_eq!(status_watcher_b.current(), ServiceStatus::Ready);
    assert_eq!(status_watcher_c.current(), ServiceStatus::Stopped);

    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().shutdown());
}

#[test]
fn test_start_all() {
    let overwatch = initialize();
    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().start_all_services());

    let status_watcher_a = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceA>());
    let status_watcher_b = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceB>());
    let status_watcher_c = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceC>());

    assert_eq!(status_watcher_a.current(), ServiceStatus::Ready);
    assert_eq!(status_watcher_b.current(), ServiceStatus::Ready);
    assert_eq!(status_watcher_c.current(), ServiceStatus::Ready);

    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().shutdown());
}

#[test]
fn test_stop() {
    let overwatch = initialize();
    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().start_all_services());

    let status_watcher_a = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceA>());
    let status_watcher_b = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceB>());
    let status_watcher_c = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceC>());

    overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().stop_service::<ServiceA>())
        .expect("Failed to stop service");

    assert_eq!(status_watcher_a.current(), ServiceStatus::Stopped);
    assert_eq!(status_watcher_b.current(), ServiceStatus::Ready);
    assert_eq!(status_watcher_c.current(), ServiceStatus::Ready);

    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().shutdown());
}

#[test]
fn test_stop_list() {
    let overwatch = initialize();
    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().start_all_services());

    let mut status_watcher_a = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceA>());
    let mut status_watcher_b = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceB>());
    let status_watcher_c = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceC>());

    let services: Vec<RuntimeServiceId> = vec![
        <RuntimeServiceId as AsServiceId<ServiceA>>::SERVICE_ID,
        <RuntimeServiceId as AsServiceId<ServiceB>>::SERVICE_ID,
    ];
    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().stop_service_sequence(services));

    // Because stop_service_list does not have a synchronisation mechanism,
    // we need to wait for the status to change, as the services may take some time
    // to stop.
    let runtime = overwatch.runtime().handle();
    wait_for_status(runtime, &mut status_watcher_a, ServiceStatus::Stopped);
    wait_for_status(runtime, &mut status_watcher_b, ServiceStatus::Stopped);

    assert_eq!(status_watcher_a.current(), ServiceStatus::Stopped);
    assert_eq!(status_watcher_b.current(), ServiceStatus::Stopped);
    assert_eq!(status_watcher_c.current(), ServiceStatus::Ready);

    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().shutdown());
}

#[test]
fn test_stop_all() {
    let overwatch = initialize();
    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().start_all_services());

    let mut status_watcher_a = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceA>());
    let mut status_watcher_b = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceB>());
    let mut status_watcher_c = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().status_watcher::<ServiceC>());

    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().stop_all_services());

    // Because stop_service_list does not have a synchronisation mechanism,
    // we need to wait for the status to change, as the services may take some time
    // to stop.
    let handle = overwatch.runtime().handle();
    wait_for_status(handle, &mut status_watcher_a, ServiceStatus::Stopped);
    wait_for_status(handle, &mut status_watcher_b, ServiceStatus::Stopped);
    wait_for_status(handle, &mut status_watcher_c, ServiceStatus::Stopped);

    assert_eq!(status_watcher_a.current(), ServiceStatus::Stopped);
    assert_eq!(status_watcher_b.current(), ServiceStatus::Stopped);
    assert_eq!(status_watcher_c.current(), ServiceStatus::Stopped);

    let _ = overwatch
        .runtime()
        .handle()
        .block_on(overwatch.handle().shutdown());
}
