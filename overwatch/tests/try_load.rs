use std::{
    sync::mpsc::{self, SendError},
    thread,
    time::Duration,
};

use async_trait::async_trait;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        state::{ServiceState, StateOperator},
        ServiceCore, ServiceData,
    },
    DynError, OpaqueServiceResourcesHandle,
};

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
    origin_sender: mpsc::Sender<String>,
}

struct TryLoad {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

impl ServiceData for TryLoad {
    type Settings = TryLoadSettings;
    type State = TryLoadState;
    type StateOperator = TryLoadOperator;
    type Message = ();
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for TryLoad {
    fn init(
        service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self {
            service_resources_handle,
        })
    }

    async fn run(self) -> Result<(), DynError> {
        let Self {
            service_resources_handle,
            ..
        } = self;
        let sender = service_resources_handle
            .settings_updater
            .notifier()
            .get_updated_settings()
            .origin_sender;
        sender.send(String::from("Service::run")).unwrap();
        service_resources_handle.overwatch_handle.shutdown().await;
        Ok(())
    }
}

#[derive_services]
struct TryLoadApp {
    try_load: TryLoad,
}

#[test]
fn load_state_from_operator() {
    // Create a sender that will be called wherever the state is loaded
    let (origin_sender, origin_receiver) = mpsc::channel();
    let settings = TryLoadAppServiceSettings {
        try_load: TryLoadSettings { origin_sender },
    };

    // Run the app
    let app = OverwatchRunner::<TryLoadApp>::run(settings, None).unwrap();
    let handle = app.handle().clone();

    handle
        .runtime()
        .block_on(handle.start_service::<TryLoad>())
        .expect("service to start successfully.");

    app.wait_finished();

    // Check if the origin was called
    thread::sleep(Duration::from_secs(1));
    let service_message_1 = origin_receiver.recv().expect("Value was not sent");
    assert_eq!(service_message_1, "StateOperator::try_load");
    let service_message_2 = origin_receiver.recv().expect("Value was not sent");
    assert_eq!(service_message_2, "Service::run");
}
