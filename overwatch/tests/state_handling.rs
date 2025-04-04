use std::{convert::Infallible, time::Duration};

use async_trait::async_trait;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        life_cycle::LifecycleMessage,
        state::{ServiceState, StateOperator},
        ServiceCore, ServiceData,
    },
    OpaqueServiceStateHandle,
};
use tokio::{
    io::{self, AsyncWriteExt},
    time::sleep,
};
use tokio_stream::StreamExt as _;

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
        state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self { state })
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        let Self {
            state:
                OpaqueServiceStateHandle::<Self, RuntimeServiceId> {
                    state_updater,
                    lifecycle_handle,
                    ..
                },
        } = self;
        let mut lifecycle_stream = lifecycle_handle.message_stream();

        let lifecycle_message = lifecycle_stream
            .next()
            .await
            .expect("first received message to be a lifecycle message.");

        let sender = match lifecycle_message {
            LifecycleMessage::Shutdown(sender) => {
                sender.send(()).unwrap();
                return Ok(());
            }
            LifecycleMessage::Kill => return Ok(()),
            // Continue below if a `Start` message is received.
            LifecycleMessage::Start(sender) => sender,
        };

        sender.send(()).unwrap();

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

    handle
        .runtime()
        .block_on(handle.start_service::<UpdateStateService>())
        .expect("service to start successfully.");

    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        handle.shutdown().await;
    });
    overwatch.wait_finished();
}
