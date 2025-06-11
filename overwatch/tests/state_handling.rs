use std::{convert::Infallible, time::Duration};

use async_trait::async_trait;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        state::{ServiceState, StateOperator},
        ServiceCore, ServiceData,
    },
    OpaqueServiceResourcesHandle,
};
use tokio::{
    io::{self, AsyncWriteExt},
    time::sleep,
};

pub struct UpdateStateService {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

#[derive(Clone, Debug)]
pub struct UpdateStateServiceMessage;

#[derive(Debug)]
pub struct UnitError;

impl core::fmt::Display for UnitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "UnitError")
    }
}

impl std::error::Error for UnitError {}

#[derive(Clone)]
pub struct CounterState {
    value: usize,
}

impl ServiceState for CounterState {
    type Settings = ();
    type Error = UnitError;

    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self { value: 0 })
    }
}

#[derive(Clone)]
pub struct CounterStateOperator;

#[async_trait]
impl StateOperator for CounterStateOperator {
    type State = CounterState;
    type LoadError = Infallible;

    fn try_load(
        _settings: &<Self::State as ServiceState>::Settings,
    ) -> Result<Option<Self::State>, Self::LoadError> {
        Ok(None)
    }

    fn from_settings(_settings: &<Self::State as ServiceState>::Settings) -> Self {
        Self
    }

    async fn run(&mut self, state: Self::State) {
        let value = state.value;
        let mut stdout = io::stdout();
        stdout
            .write_all(format!("Updated state value received: {value}\n").as_bytes())
            .await
            .expect("stop Output wrote");
        assert!(value < 10);
    }
}

impl ServiceData for UpdateStateService {
    type Settings = ();
    type State = CounterState;
    type StateOperator = CounterStateOperator;
    type Message = UpdateStateServiceMessage;
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for UpdateStateService {
    fn init(
        service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self {
            service_resources_handle,
        })
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        let state_updater = self.service_resources_handle.state_updater;
        for value in 0..10 {
            state_updater.update(Some(CounterState { value }));
            sleep(Duration::from_millis(50)).await;
        }
        Ok(())
    }
}

#[derive_services]
struct TestApp {
    update_state_service: UpdateStateService,
}

#[test]
fn state_update_service() {
    let settings: TestAppServiceSettings = TestAppServiceSettings {
        update_state_service: (),
    };
    let overwatch = OverwatchRunner::<TestApp>::run(settings, None).unwrap();
    let handle = overwatch.handle().clone();

    handle
        .runtime()
        .block_on(handle.start_service::<UpdateStateService>())
        .expect("service to start successfully.");

    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        let _ = handle.shutdown().await;
    });
    overwatch.wait_finished();
}
