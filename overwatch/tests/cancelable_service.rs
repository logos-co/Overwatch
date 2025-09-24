use std::time::Duration;

use overwatch::{
    DynError, OpaqueServiceResourcesHandle, derive_services,
    overwatch::{
        OverwatchRunner,
        commands::{OverwatchCommand, ServiceLifecycleCommand, ServiceSingleCommand},
    },
    services::{
        AsServiceId as _, ServiceCore, ServiceData,
        state::{NoOperator, NoState},
    },
};
use tokio::time::sleep;

pub struct CancellableService;

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

        #[expect(clippy::non_ascii_literal, reason = "We like skulls")]
        loop {
            println!("Waiting to be killed 💀");
            cumulative_time += interval.tick().await.elapsed();
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
    let (sender, receiver) = tokio::sync::oneshot::channel();
    overwatch.spawn(async move {
        sleep(Duration::from_millis(500)).await;
        let command = OverwatchCommand::ServiceLifecycle(ServiceLifecycleCommand::StopService(
            ServiceSingleCommand {
                service_id: RuntimeServiceId::SERVICE_ID,
                sender,
            },
        ));

        let _ = handle.send(command).await;
        // Wait until the service is stopped
        receiver.await.unwrap();

        let _ = handle.shutdown().await;
    });
    overwatch.blocking_wait_finished();
}
