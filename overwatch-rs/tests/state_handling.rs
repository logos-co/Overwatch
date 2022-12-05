use async_trait::async_trait;
use overwatch_derive::Services;
use overwatch_rs::overwatch::OverwatchRunner;
use overwatch_rs::services::handle::{ServiceHandle, ServiceStateHandle};
use overwatch_rs::services::relay::RelayMessage;
use overwatch_rs::services::state::{ServiceState, StateOperator};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use std::time::Duration;
use tokio::io::{self, AsyncWriteExt};
use tokio::time::sleep;

pub struct UpdateStateService {
    state: ServiceStateHandle<Self>,
}

#[derive(Clone, Debug)]
pub struct UpdateStateServiceMessage(String);

impl RelayMessage for UpdateStateServiceMessage {}

#[derive(Clone)]
pub struct CounterState {
    value: usize,
}

impl ServiceState for CounterState {
    type Settings = ();

    fn from_settings(_settings: &Self::Settings) -> Self {
        Self { value: 0 }
    }
}

#[derive(Clone)]
pub struct CounterStateOperator;

#[async_trait]
impl StateOperator for CounterStateOperator {
    type StateInput = CounterState;

    fn from_settings<Settings>(_settings: Settings) -> Self {
        CounterStateOperator
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
    const SERVICE_ID: ServiceId = "FooService";
    type Settings = ();
    type State = CounterState;
    type StateOperator = CounterStateOperator;
    type Message = UpdateStateServiceMessage;
}

#[async_trait]
impl ServiceCore for UpdateStateService {
    fn init(state: ServiceStateHandle<Self>) -> Self {
        Self { state }
    }

    async fn run(mut self) {
        let Self {
            state: ServiceStateHandle { state_updater, .. },
        } = self;
        for value in 0..10 {
            state_updater.update(CounterState { value });
            sleep(Duration::from_millis(50)).await;
        }
    }
}

#[derive(Services)]
struct TestApp {
    update_state_service: ServiceHandle<UpdateStateService>,
}

#[test]
fn state_update_service() {
    let settings: TestAppServiceSettings = TestAppServiceSettings {
        update_state_service: (),
    };
    let overwatch = OverwatchRunner::<TestApp>::run(settings, None);
    let handle = overwatch.handle().clone();

    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        handle.shutdown().await;
    });
    overwatch.wait_finished();
}
