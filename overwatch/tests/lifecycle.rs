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
use tokio::{runtime::Handle, sync::oneshot};

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
        self.save_finished_signal_sender.send(()).unwrap();
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
        assert_sender.send(initial_state.value.to_string()).unwrap();

        // Increment and save
        let value = initial_state.value + 1;
        service_resources_handle
            .state_updater
            .update(Some(Self::State { value }));

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

    // When a Service is stopped, its StateHandler is stopped as well, which
    // includes the StateOperator.
    // Due to this, and to achieve test idempotency, we wait until StateOperator
    // finishes running before proceeding to the next step.
    let (state_operator_save_finished_signal_sender, state_operator_save_finished_signal_receiver) =
        channel();

    let (assert_sender, assert_receiver) = channel();
    let settings = AppServiceSettings {
        lifecycle_service: LifecycleServiceSettings {
            assert_sender,
            saved_state: &SAVED_STATE,
            state_operator_save_finished_signal_sender,
        },
    };

    let app = OverwatchRunner::<App>::run(settings, None).unwrap();
    let handle = app.handle();
    let runtime = handle.runtime();

    // Start the Service
    let (lifecycle_sender, lifecycle_receiver) = oneshot::channel();
    send_lifecycle_message(runtime, handle, LifecycleMessage::Start(lifecycle_sender));
    runtime.block_on(lifecycle_receiver).unwrap();

    // To avoid test failures, wait until StateOperator has saved the initial state
    // from the ServiceRunner
    state_operator_save_finished_signal_receiver.recv().unwrap();

    // Check the initial value is sent from within the Service
    let service_value = assert_receiver.recv().unwrap();
    assert_eq!(service_value, "0");

    // To avoid test failures, wait until StateOperator has saved the state from the
    // Service
    state_operator_save_finished_signal_receiver.recv().unwrap();

    // Stop the Service
    let (lifecycle_sender, lifecycle_receiver) = oneshot::channel();
    send_lifecycle_message(runtime, handle, LifecycleMessage::Stop(lifecycle_sender));
    runtime.block_on(lifecycle_receiver).unwrap();

    // Check that the Service hasn't sent any messages
    assert_receiver.try_recv().unwrap_err();

    // Start the Service again
    let (lifecycle_sender, lifecycle_receiver) = oneshot::channel();
    send_lifecycle_message(runtime, handle, LifecycleMessage::Start(lifecycle_sender));
    runtime.block_on(lifecycle_receiver).unwrap();

    // To avoid test failures, wait until StateOperator has saved the initial state
    // from the ServiceRunner
    state_operator_save_finished_signal_receiver.recv().unwrap();

    // Check the initial value is sent from within the Service
    let service_value = assert_receiver.recv().unwrap();
    assert_eq!(service_value, "1");

    // To avoid test failures, wait until StateOperator has saved to send the state
    // from the Service
    state_operator_save_finished_signal_receiver.recv().unwrap();

    // Stop the Service again
    let (lifecycle_sender, lifecycle_receiver) = oneshot::channel();
    send_lifecycle_message(runtime, handle, LifecycleMessage::Stop(lifecycle_sender));
    runtime.block_on(lifecycle_receiver).unwrap();

    // Check that the Service hasn't sent any messages
    assert_receiver.try_recv().unwrap_err();

    // Check the last saved value
    let state_value = {
        let saved_state_guard = SAVED_STATE.lock().unwrap();
        saved_state_guard.as_ref().unwrap().value
    }; // MutexGuard is dropped here, before the .await
    assert_eq!(state_value, 2);

    runtime.block_on(handle.shutdown());
    app.wait_finished();
}
