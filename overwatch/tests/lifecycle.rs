use std::{
    convert::Infallible,
    sync::{
        mpsc::{channel, Sender},
        Mutex,
    },
};

use async_trait::async_trait;
use overwatch::{
    overwatch::{
        commands::{OverwatchCommand, ServiceLifeCycleCommand},
        handle::OverwatchHandle,
        OverwatchRunner,
    },
    services::{
        life_cycle::LifecycleMessage,
        resources::ServiceResourcesHandle,
        state::{ServiceState, StateOperator},
        ServiceCore, ServiceData,
    },
    DynError, OpaqueServiceResourcesHandle,
};
use overwatch_derive::derive_services;
use tokio::{runtime::Handle, sync::broadcast};

#[derive(Debug, Clone)]
struct LifecycleServiceState {
    value: u8,
}

impl ServiceState for LifecycleServiceState {
    type Settings = LifecycleServiceSettings;
    type Error = Infallible;

    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self { value: 0 })
    }
}

#[derive(Clone)]
struct LifecycleServiceStateOperator {
    saved_state: &'static Mutex<Option<LifecycleServiceState>>,
}

#[async_trait]
impl StateOperator for LifecycleServiceStateOperator {
    type State = LifecycleServiceState;
    type LoadError = String;

    fn try_load(
        settings: &<Self::State as ServiceState>::Settings,
    ) -> Result<Option<Self::State>, Self::LoadError> {
        settings
            .saved_state
            .try_lock()
            .map(|mut saved_state| saved_state.take())
            .map_err(|_| "Failed to lock the saved state mutex.".to_string())
    }

    fn from_settings(settings: &<Self::State as ServiceState>::Settings) -> Self {
        Self {
            saved_state: settings.saved_state,
        }
    }

    async fn run(&mut self, state: Self::State) {
        if let Ok(mut lock) = self.saved_state.lock() {
            *lock = Some(state);
        } else {
            panic!("Failed to lock saved state mutex.");
        }
    }
}

#[derive(Debug, Clone)]
struct LifecycleServiceSettings {
    sender: Sender<String>,
    saved_state: &'static Mutex<Option<LifecycleServiceState>>,
}

struct LifecycleService {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
    initial_state: <Self as ServiceData>::State,
}

impl ServiceData for LifecycleService {
    type Settings = LifecycleServiceSettings;
    type State = LifecycleServiceState;
    type StateOperator = LifecycleServiceStateOperator;
    type Message = ();
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for LifecycleService {
    fn init(
        service_resources_handle: ServiceResourcesHandle<
            Self::Message,
            Self::Settings,
            Self::State,
            RuntimeServiceId,
        >,
        initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self {
            service_resources_handle,
            initial_state,
        })
    }

    async fn run(self) -> Result<(), DynError> {
        let Self {
            service_resources_handle,
            initial_state,
        } = self;

        let sender = service_resources_handle
            .settings_updater
            .notifier()
            .get_updated_settings()
            .sender;

        // Initial value
        sender.send(initial_state.value.to_string()).unwrap();

        // Increment and save
        let value = initial_state.value + 1;
        service_resources_handle
            .state_updater
            .update(Self::State { value });

        Ok(())
    }
}

#[derive_services]
struct App {
    lifecycle_service: LifecycleService,
}

fn send_lifecycle_message(
    runtime: &Handle,
    handle: &OverwatchHandle<RuntimeServiceId>,
    msg: LifecycleMessage,
) {
    runtime
        .block_on(handle.send(OverwatchCommand::ServiceLifeCycle(
            ServiceLifeCycleCommand {
                service_id: RuntimeServiceId::LifecycleService,
                msg,
            },
        )))
        .unwrap();
}

#[test]
fn test_lifecycle() {
    static SAVED_STATE: Mutex<Option<LifecycleServiceState>> = Mutex::new(None);

    let (service_sender, service_receiver) = channel();
    let settings = AppServiceSettings {
        lifecycle_service: LifecycleServiceSettings {
            sender: service_sender,
            saved_state: &SAVED_STATE,
        },
    };

    let app = OverwatchRunner::<App>::run(settings, None).unwrap();
    let handle = app.handle();
    let runtime = handle.runtime();
    let (lifecycle_sender, mut lifecycle_receiver) = broadcast::channel(1);

    // Start Service
    send_lifecycle_message(
        runtime,
        handle,
        LifecycleMessage::Start(lifecycle_sender.clone()),
    );

    runtime.block_on(lifecycle_receiver.recv()).unwrap();
    let service_value = service_receiver.recv().unwrap();
    assert_eq!(service_value, "0");

    // Shutdown Service
    send_lifecycle_message(
        runtime,
        handle,
        LifecycleMessage::Stop(lifecycle_sender.clone()),
    );
    runtime.block_on(lifecycle_receiver.recv()).unwrap();
    service_receiver.try_recv().unwrap_err();

    // Start Service again
    send_lifecycle_message(
        runtime,
        handle,
        LifecycleMessage::Start(lifecycle_sender.clone()),
    );
    runtime.block_on(lifecycle_receiver.recv()).unwrap();
    let service_value = service_receiver.recv().unwrap();
    assert_eq!(service_value, "1");

    // Shutdown Service again
    send_lifecycle_message(runtime, handle, LifecycleMessage::Stop(lifecycle_sender));
    runtime.block_on(lifecycle_receiver.recv()).unwrap();
    service_receiver.try_recv().unwrap_err();
}
