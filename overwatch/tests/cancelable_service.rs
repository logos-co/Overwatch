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
    DynError, OpaqueServiceStateHandle,
};
use tokio::time::sleep;
use tokio_stream::StreamExt;

pub struct CancellableService {
    service_state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
}

impl ServiceData for CancellableService {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State, Self::Settings>;
    type Message = ();
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for CancellableService {
    fn init(
        service_state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), DynError> {
        let mut lifecycle_stream = self.service_state.lifecycle_handle.message_stream();
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        #[expect(clippy::redundant_pub_crate)]
        loop {
            tokio::select! {
                msg = lifecycle_stream.next() => {
                    match msg {
                        Some(LifecycleMessage::Shutdown(reply)) => {
                            reply.send(()).unwrap();
                            break;
                        }
                        Some(LifecycleMessage::Kill) => {
                            break;
                        }
                        _ => {
                            unimplemented!();
                        }
                    }
                }
                _ =  interval.tick() =>  {
                    println!("Waiting to be killed 💀");
                }
            }
        }
        Ok(())
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
