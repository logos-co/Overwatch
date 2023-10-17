use overwatch_derive::Services;
use overwatch_rs::overwatch::OverwatchRunner;
use overwatch_rs::services::handle::{ServiceHandle, ServiceStateHandle};
use overwatch_rs::services::relay::{NoMessage, Relay};
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use overwatch_rs::DynError;
use std::time::Duration;
use tracing::debug;

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
    type StateOperator = NoOperator<Self::State>;
    type Message = NoMessage;
}

impl ServiceData for AwaitService2 {
    const SERVICE_ID: ServiceId = "S2";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = NoMessage;
}

impl ServiceData for AwaitService3 {
    const SERVICE_ID: ServiceId = "S3";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = NoMessage;
}
#[async_trait::async_trait]
impl ServiceCore for AwaitService1 {
    fn init(service_state: ServiceStateHandle<Self>) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        debug!("Initialized 1");
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ServiceCore for AwaitService2 {
    fn init(service_state: ServiceStateHandle<Self>) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        let relay: Relay<AwaitService1> = self.service_state.overwatch_handle.relay();
        relay
            .connect()
            .await
            .expect("Connection from 2 to 1 couldn't be initialized");
        debug!("Initialized 2");
        Ok(())
    }
}

#[async_trait::async_trait]
impl ServiceCore for AwaitService3 {
    fn init(service_state: ServiceStateHandle<Self>) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        let relay: Relay<AwaitService2> = self.service_state.overwatch_handle.relay();
        relay
            .connect()
            .await
            .expect("Connection from 2 to 1 couldn't be initialized");
        debug!("Initialized 3");
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
fn run_overwatch_then_shutdown_service_and_kill() {
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
