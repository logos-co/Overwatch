use overwatch::overwatch::OverwatchRunner;
use overwatch::services::relay::NoMessage;
use overwatch::services::state::{NoOperator, NoState};
use overwatch::services::status::{ServiceStatus, StatusWatcher};
use overwatch::services::{ServiceCore, ServiceData, ServiceId};
use overwatch::DynError;
use overwatch::{OpaqueServiceHandle, OpaqueServiceStateHandle};
use overwatch_derive::Services;
use std::time::Duration;

pub struct AwaitService1 {
    service_state: OpaqueServiceStateHandle<Self>,
}

pub struct AwaitService2 {
    service_state: OpaqueServiceStateHandle<Self>,
}

pub struct AwaitService3 {
    service_state: OpaqueServiceStateHandle<Self>,
}

impl ServiceData for AwaitService1 {
    const SERVICE_ID: ServiceId = "S1";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State, Self::Settings>;
    type Message = NoMessage;
}

impl ServiceData for AwaitService2 {
    const SERVICE_ID: ServiceId = "S2";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State, Self::Settings>;
    type Message = NoMessage;
}

impl ServiceData for AwaitService3 {
    const SERVICE_ID: ServiceId = "S3";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State, Self::Settings>;
    type Message = NoMessage;
}

#[async_trait::async_trait]
impl ServiceCore for AwaitService1 {
    fn init(
        service_state: OpaqueServiceStateHandle<Self>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        println!("Initialized 1");
        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Running);
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Stopped);
        Ok(())
    }
}

#[async_trait::async_trait]
impl ServiceCore for AwaitService2 {
    fn init(
        service_state: OpaqueServiceStateHandle<Self>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Running);

        let mut watcher: StatusWatcher = self
            .service_state
            .overwatch_handle
            .status_watcher::<AwaitService1>()
            .await;

        watcher
            .wait_for(ServiceStatus::Running, Some(Duration::from_millis(50)))
            .await
            .unwrap();

        println!("Initialized 2");
        tokio::time::sleep(Duration::from_millis(100)).await;
        watcher
            .wait_for(ServiceStatus::Stopped, Some(Duration::from_millis(50)))
            .await
            .unwrap();
        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Stopped);
        Ok(())
    }
}

#[async_trait::async_trait]
impl ServiceCore for AwaitService3 {
    fn init(
        service_state: OpaqueServiceStateHandle<Self>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Running);

        let mut watcher: StatusWatcher = self
            .service_state
            .overwatch_handle
            .status_watcher::<AwaitService2>()
            .await;

        watcher
            .wait_for(ServiceStatus::Running, Some(Duration::from_millis(50)))
            .await
            .unwrap();

        println!("Initialized 3");
        tokio::time::sleep(Duration::from_millis(100)).await;
        watcher
            .wait_for(ServiceStatus::Stopped, Some(Duration::from_millis(50)))
            .await
            .unwrap();
        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Stopped);
        Ok(())
    }
}

#[derive(Services)]
struct SequenceServices {
    c: OpaqueServiceHandle<AwaitService3>,
    b: OpaqueServiceHandle<AwaitService2>,
    a: OpaqueServiceHandle<AwaitService1>,
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

    overwatch.spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        handle.shutdown().await;
    });
    overwatch.wait_finished();
}
