use std::{thread, time::Duration};

use async_trait::async_trait;
use overwatch::{
    overwatch::OverwatchRunner,
    services::{
        relay::NoMessage,
        state::{ServiceState, StateOperator},
        ServiceCore, ServiceData, ServiceId,
    },
    DynError, OpaqueServiceHandle, OpaqueServiceStateHandle,
};
use overwatch_derive::Services;
use tokio::sync::{broadcast, broadcast::error::SendError};

#[derive(Clone)]
struct TryLoadState;

impl ServiceState for TryLoadState {
    type Settings = TryLoadSettings;
    type Error = DynError;
    fn from_settings(settings: &Self::Settings) -> Result<Self, DynError> {
        settings
            .origin_sender
            .send(String::from("ServiceState::from_settings"))?;
        Ok(Self {})
    }
}

#[derive(Clone)]
struct TryLoadOperator;

#[async_trait]
impl StateOperator for TryLoadOperator {
    type State = TryLoadState;
    type LoadError = SendError<String>;

    fn try_load(
        settings: &<Self::State as ServiceState>::Settings,
    ) -> Result<Option<Self::State>, Self::LoadError> {
        settings
            .origin_sender
            .send(String::from("StateOperator::try_load"))?;
        Ok(Some(Self::State {}))
    }

    fn from_settings(_settings: &<Self::State as ServiceState>::Settings) -> Self {
        Self {}
    }

    async fn run(&mut self, _state: Self::State) {}
}

#[derive(Debug, Clone)]
struct TryLoadSettings {
    origin_sender: broadcast::Sender<String>,
}

struct TryLoad {
    service_state_handle: OpaqueServiceStateHandle<Self>,
}

impl ServiceData for TryLoad {
    const SERVICE_ID: ServiceId = "try_load";
    type Settings = TryLoadSettings;
    type State = TryLoadState;
    type StateOperator = TryLoadOperator;
    type Message = NoMessage;
}

#[async_trait]
impl ServiceCore for TryLoad {
    fn init(
        service_state: OpaqueServiceStateHandle<Self>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self {
            service_state_handle: service_state,
        })
    }

    async fn run(self) -> Result<(), DynError> {
        let Self {
            service_state_handle,
            ..
        } = self;

        service_state_handle.overwatch_handle.shutdown().await;
        Ok(())
    }
}

#[derive(Services)]
struct TryLoadApp {
    try_load: OpaqueServiceHandle<TryLoad>,
}

#[test]
fn load_state_from_operator() {
    // Create a sender that will be called wherever the state is loaded
    let (origin_sender, mut origin_receiver) = broadcast::channel(1);
    let settings = TryLoadAppServiceSettings {
        try_load: TryLoadSettings { origin_sender },
    };

    // Run the app
    let app = OverwatchRunner::<TryLoadApp>::run(settings, None).unwrap();
    app.wait_finished();

    // Check if the origin was called
    thread::sleep(Duration::from_secs(1));
    let origin = origin_receiver.try_recv().expect("Value was not sent");
    assert_eq!(origin, "StateOperator::try_load");
}
