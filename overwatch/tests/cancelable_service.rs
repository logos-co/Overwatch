use std::time::Duration;

use overwatch::{
    overwatch::{
        commands::{OverwatchCommand, ServiceLifeCycleCommand},
        OverwatchRunner,
    },
    services::{
        life_cycle::LifecycleMessage,
        relay::NoMessage,
        state::{NoOperator, NoState},
        ServiceCore, ServiceData, ServiceId,
    },
    DynError, OpaqueServiceHandle, OpaqueServiceStateHandle,
};
use overwatch_derive::Services;
use tokio::time::sleep;
use tokio_stream::StreamExt;

pub struct CancellableService {
    service_state: OpaqueServiceStateHandle<Self>,
}

impl ServiceData for CancellableService {
    const SERVICE_ID: ServiceId = "cancel-me-please";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State, Self::Settings>;
    type Message = NoMessage;
}

#[async_trait::async_trait]
impl ServiceCore for CancellableService {
    fn init(
        service_state: OpaqueServiceStateHandle<Self>,
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

#[derive(Services)]
struct CancelableServices {
    cancelable: OpaqueServiceHandle<CancellableService>,
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
                    service_id: <CancellableService as ServiceData>::SERVICE_ID,
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
