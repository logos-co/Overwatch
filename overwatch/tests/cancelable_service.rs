use std::time::Duration;

use overwatch::{
    derive_services,
    overwatch::{
        commands::{OverwatchCommand, ServiceLifeCycleCommand},
        OverwatchRunner,
    },
    services::{
        life_cycle::LifecycleMessage,
        state::{NoOperator, NoState},
        AsServiceId, ServiceCore, ServiceData,
    },
    DynError, OpaqueServiceResourcesHandle,
};
use tokio::time::sleep;

pub struct CancellableService {}

impl ServiceData for CancellableService {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for CancellableService {
    fn init(
        _service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self {})
    }

    async fn run(self) -> Result<(), DynError> {
        let mut cumulative_time = Duration::from_millis(0);
        let mut interval = tokio::time::interval(Duration::from_millis(200));

        loop {
            let x = interval.tick().await;
            println!("Waiting to be killed 💀");
            cumulative_time += x.elapsed();
            assert!(
                cumulative_time <= Duration::from_secs(2),
                "Timeout while waiting to be killed."
            );
        }
    }
}

#[derive_services]
struct CancelableServices {
    cancelable: CancellableService,
}

#[test]
fn run_overwatch_then_shutdown_service_and_kill() {
    let settings = CancelableServicesServiceSettings { cancelable: () };
    let overwatch = OverwatchRunner::<CancelableServices>::run(settings, None).unwrap();
    let handle = overwatch.handle().clone();
    handle
        .runtime()
        .block_on(handle.start_service::<CancellableService>())
        .expect("service to start successfully.");
    let (sender, mut receiver) = tokio::sync::broadcast::channel(1);
    overwatch.spawn(async move {
        sleep(Duration::from_millis(500)).await;
        let _ = handle
            .send(OverwatchCommand::ServiceLifeCycle(
                ServiceLifeCycleCommand {
                    service_id: RuntimeServiceId::SERVICE_ID,
                    msg: LifecycleMessage::Shutdown(sender),
                },
            ))
            .await;
        // wait service finished
        receiver.recv().await.unwrap();
        handle.kill().await;
    });
    overwatch.wait_finished();
}
