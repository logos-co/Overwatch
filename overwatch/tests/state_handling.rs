use std::{convert::Infallible, time::Duration};

use async_trait::async_trait;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        state::{ServiceState, StateOperator},
        ServiceCore, ServiceData,
    },
    OpaqueServiceStateHandle,
};
use tokio::{
    io::{self, AsyncWriteExt},
    time::sleep,
};

pub struct UpdateStateService {
    state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
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
    type StateInput = CounterState;
    type Settings = ();
    type LoadError = Infallible;

    fn try_load(
        _settings: &<Self::StateInput as ServiceState>::Settings,
    ) -> Result<Option<Self::StateInput>, Self::LoadError> {
        Ok(None)
    }

    fn from_settings(_settings: <Self::StateInput as ServiceState>::Settings) -> Self {
        Self
    }

    async fn run(&mut self, state: Self::StateInput) {
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
    const SERVICE_NAME: &'static str = "FooService";
    type Settings = ();
    type State = CounterState;
    type StateOperator = CounterStateOperator;
    type Message = UpdateStateServiceMessage;
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for UpdateStateService {
    fn init(
        state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self { state })
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        let Self {
            state: OpaqueServiceStateHandle::<Self, RuntimeServiceId> { state_updater, .. },
        } = self;
        for value in 0..10 {
            state_updater.update(CounterState { value });
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

    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        handle.shutdown().await;
    });
    overwatch.wait_finished();
}
