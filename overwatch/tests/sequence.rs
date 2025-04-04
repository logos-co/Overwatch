use std::time::Duration;

use futures::future::join3;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        life_cycle::LifecycleMessage,
        state::{NoOperator, NoState},
        status::{ServiceStatus, StatusWatcher},
        ServiceCore, ServiceData,
    },
    DynError, OpaqueServiceStateHandle,
};
use tokio_stream::StreamExt as _;

pub struct AwaitService1 {
    service_state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
}

pub struct AwaitService2 {
    service_state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
}

pub struct AwaitService3 {
    service_state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
}

impl ServiceData for AwaitService1 {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

impl ServiceData for AwaitService2 {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

impl ServiceData for AwaitService3 {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for AwaitService1 {
    fn init(
        service_state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        println!("Initialized 1");
        let mut lifecycle_stream = self.service_state.lifecycle_handle.message_stream();

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

        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Running);
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Stopped);
        Ok(())
    }
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for AwaitService2 {
    fn init(
        service_state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        let mut lifecycle_stream = self.service_state.lifecycle_handle.message_stream();

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

        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Running);

        let mut watcher: StatusWatcher = self
            .service_state
            .overwatch_handle
            .status_watcher::<AwaitService1>()
            .await;

        watcher
            .wait_for(ServiceStatus::Running, Some(Duration::from_millis(50)))
            .await
            .unwrap();

        println!("Initialized 2");
        tokio::time::sleep(Duration::from_millis(100)).await;
        watcher
            .wait_for(ServiceStatus::Stopped, Some(Duration::from_millis(50)))
            .await
            .unwrap();
        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Stopped);
        Ok(())
    }
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for AwaitService3 {
    fn init(
        service_state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        let mut lifecycle_stream = self.service_state.lifecycle_handle.message_stream();

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

        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Running);

        let mut watcher: StatusWatcher = self
            .service_state
            .overwatch_handle
            .status_watcher::<AwaitService2>()
            .await;

        watcher
            .wait_for(ServiceStatus::Running, Some(Duration::from_millis(50)))
            .await
            .unwrap();

        println!("Initialized 3");
        tokio::time::sleep(Duration::from_millis(100)).await;
        watcher
            .wait_for(ServiceStatus::Stopped, Some(Duration::from_millis(50)))
            .await
            .unwrap();
        self.service_state
            .status_handle
            .updater()
            .update(ServiceStatus::Stopped);
        Ok(())
    }
}

#[derive_services]
struct SequenceServices {
    c: AwaitService3,
    b: AwaitService2,
    a: AwaitService1,
}

#[test]
fn sequenced_services_startup() {
    let settings = SequenceServicesServiceSettings {
        a: (),
        b: (),
        c: (),
    };
    let overwatch = OverwatchRunner::<SequenceServices>::run(settings, None).unwrap();
    let handle = overwatch.handle().clone();

    let _ = handle.runtime().block_on(join3(
        handle.start_service::<AwaitService1>(),
        handle.start_service::<AwaitService2>(),
        handle.start_service::<AwaitService3>(),
    ));

    overwatch.spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        handle.shutdown().await;
    });
    overwatch.wait_finished();
}
