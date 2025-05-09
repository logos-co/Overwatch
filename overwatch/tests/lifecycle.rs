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
use std::{convert::Infallible, sync::Mutex};
use tokio::sync::mpsc::{channel, Sender};
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
    save_finished_signal_sender: Sender<()>,
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
            save_finished_signal_sender: settings
                .state_operator_save_finished_signal_sender
                .clone(),
        }
    }

    async fn run(&mut self, state: Self::State) {
        if let Ok(mut lock) = self.saved_state.lock() {
            *lock = Some(state);
        } else {
            panic!("Failed to lock saved state mutex.");
        }
        self.save_finished_signal_sender.send(()).await.unwrap();
    }
}

#[derive(Debug, Clone)]
struct LifecycleServiceSettings {
    assert_sender: Sender<String>,
    saved_state: &'static Mutex<Option<LifecycleServiceState>>,
    state_operator_save_finished_signal_sender: Sender<()>,
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

        let assert_sender = service_resources_handle
            .settings_updater
            .notifier()
            .get_updated_settings()
            .assert_sender;

        // Initial value
        assert_sender
            .send(initial_state.value.to_string())
            .await
            .unwrap();

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

async fn send_lifecycle_message(
    runtime: &Handle,
    handle: &OverwatchHandle<RuntimeServiceId>,
    msg: LifecycleMessage,
) {
    handle
        .send(OverwatchCommand::ServiceLifeCycle(
            ServiceLifeCycleCommand {
                service_id: RuntimeServiceId::LifecycleService,
                msg,
            },
        ))
        .await
        .unwrap();
}

#[test]
fn test_lifecycle() {
    println!("[test_lifecycle] >>>");
    static SAVED_STATE: Mutex<Option<LifecycleServiceState>> = Mutex::new(None);
    println!("[test_lifecycle] static");
    let (
        state_operator_save_finished_signal_sender,
        mut state_operator_save_finished_signal_receiver,
    ) = channel(5);
    println!("[test_lifecycle] finished signal channel");

    let (assert_sender, mut assert_receiver) = channel(5);
    println!("[test_lifecycle] assert channel");
    let settings = AppServiceSettings {
        lifecycle_service: LifecycleServiceSettings {
            assert_sender,
            saved_state: &SAVED_STATE,
            state_operator_save_finished_signal_sender,
        },
    };
    println!("[test_lifecycle] settings");

    let app = OverwatchRunner::<App>::run(settings, None).unwrap();
    println!("[test_lifecycle] app");
    let handle = app.handle();
    let test = async move {
        println!("[test_lifecycle] handle");
        let runtime = handle.runtime();
        println!("[test_lifecycle] runtime");
        let (lifecycle_sender, mut lifecycle_receiver) = broadcast::channel(5);
        println!("[test_lifecycle] lifecycle channel");

        // Start the Service
        send_lifecycle_message(
            runtime,
            handle,
            LifecycleMessage::Start(lifecycle_sender.clone()),
        )
        .await;
        println!("[test_lifecycle] start lifecycle message");
        lifecycle_receiver.recv().await.unwrap();
        println!("[test_lifecycle] lifecycle message confirmation");

        // To avoid test failures, wait until StateOperator has saved the initial state from the ServiceRunner
        state_operator_save_finished_signal_receiver
            .recv()
            .await
            .unwrap();

        // Check the initial value is sent from within the Service
        let service_value = assert_receiver.recv().await.unwrap();
        assert_eq!(service_value, "0");

        // To avoid test failures, wait until StateOperator has saved the state from the Service
        state_operator_save_finished_signal_receiver
            .recv()
            .await
            .unwrap();

        // Stop the Service
        send_lifecycle_message(
            runtime,
            handle,
            LifecycleMessage::Stop(lifecycle_sender.clone()),
        )
        .await;
        lifecycle_receiver.recv().await.unwrap();

        // Check that the Service hasn't sent any messages
        assert_receiver.try_recv().unwrap_err();

        // Start the Service again
        send_lifecycle_message(
            runtime,
            handle,
            LifecycleMessage::Start(lifecycle_sender.clone()),
        )
        .await;
        lifecycle_receiver.recv().await.unwrap();

        // To avoid test failures, wait until StateOperator has saved the initial state from the ServiceRunner
        state_operator_save_finished_signal_receiver
            .recv()
            .await
            .unwrap();

        // Check the initial value is sent from within the Service
        let service_value = assert_receiver.recv().await.unwrap();
        assert_eq!(service_value, "1");

        // To avoid test failures, wait until StateOperator has saved to send the state from the Service
        state_operator_save_finished_signal_receiver
            .recv()
            .await
            .unwrap();

        // Stop the Service again
        send_lifecycle_message(runtime, handle, LifecycleMessage::Stop(lifecycle_sender)).await;
        lifecycle_receiver.recv().await.unwrap();

        // Check that the Service hasn't sent any messages
        assert_receiver.try_recv().unwrap_err();

        // Check the last saved value
        let state_value = {
            let saved_state_guard = SAVED_STATE.lock().unwrap();
            saved_state_guard.as_ref().unwrap().value
        }; // MutexGuard is dropped here, before the .await
        assert_eq!(state_value, 2);

        handle.shutdown().await;
    };

    app.runtime().block_on(test);
    app.wait_finished();
}
