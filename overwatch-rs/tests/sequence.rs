use overwatch_derive::Services;
use overwatch_rs::overwatch::OverwatchRunner;
use overwatch_rs::services::relay::NoMessage;
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::status::{ServiceStatus, StatusWatcher};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use overwatch_rs::DynError;
use overwatch_rs::{ServiceHandle, ServiceStateHandle};
use std::time::Duration;

pub struct AwaitService1 {
    service_state: ServiceStateHandle<Self>,
}

pub struct AwaitService2 {
    service_state: ServiceStateHandle<Self>,
}

pub struct AwaitService3 {
    service_state: ServiceStateHandle<Self>,
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
        service_state: ServiceStateHandle<Self>,
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
        service_state: ServiceStateHandle<Self>,
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
        service_state: ServiceStateHandle<Self>,
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
    c: ServiceHandle<AwaitService3>,
    b: ServiceHandle<AwaitService2>,
    a: ServiceHandle<AwaitService1>,
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
