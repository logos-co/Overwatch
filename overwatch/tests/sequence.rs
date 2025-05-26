use std::time::Duration;

use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        state::{NoOperator, NoState},
        status::{ServiceStatus, StatusWatcher},
        ServiceCore, ServiceData,
    },
    DynError, OpaqueServiceResourcesHandle,
};

pub struct AwaitService1 {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

pub struct AwaitService2 {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

pub struct AwaitService3 {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

impl ServiceData for AwaitService1 {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

impl ServiceData for AwaitService2 {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

impl ServiceData for AwaitService3 {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for AwaitService1 {
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
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for AwaitService2 {
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

        let mut watcher: StatusWatcher = self
            .service_resources_handle
            .overwatch_handle
            .status_watcher::<AwaitService1>()
            .await;

        watcher
            .wait_for(ServiceStatus::Ready, Some(Duration::from_millis(50)))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        watcher
            .wait_for(ServiceStatus::Stopped, Some(Duration::from_millis(50)))
            .await
            .unwrap();

        Ok(())
    }
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for AwaitService3 {
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

        let mut watcher: StatusWatcher = self
            .service_resources_handle
            .overwatch_handle
            .status_watcher::<AwaitService2>()
            .await;

        watcher
            .wait_for(ServiceStatus::Ready, Some(Duration::from_millis(50)))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        watcher
            .wait_for(ServiceStatus::Stopped, Some(Duration::from_millis(50)))
            .await
            .unwrap();

        Ok(())
    }
}

#[derive_services]
struct SequenceServices {
    c: AwaitService3,
    b: AwaitService2,
    a: AwaitService1,
}

#[test]
fn sequenced_services_startup() {
    let settings = SequenceServicesServiceSettings {
        a: (),
        b: (),
        c: (),
    };
    let overwatch = OverwatchRunner::<SequenceServices>::run(settings, None).unwrap();
    let handle = overwatch.handle().clone();

    handle.runtime().block_on(handle.start_all_services());

    overwatch.spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        handle.shutdown().await;
    });
    overwatch.wait_finished();
}
