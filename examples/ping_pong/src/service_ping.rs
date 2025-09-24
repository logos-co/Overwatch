use std::time::Duration;

use overwatch::{
    DynError, OpaqueServiceResourcesHandle,
    services::{ServiceCore, ServiceData},
};
use tokio::time::sleep;

use crate::{
    RuntimeServiceId,
    messages::{PingMessage, PongMessage},
    operators::StateSaveOperator,
    service_pong::PongService,
    settings::PingSettings,
    states::PingState,
};

pub struct PingService {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
    initial_state: <Self as ServiceData>::State,
}

impl ServiceData for PingService {
    type Settings = PingSettings;
    type State = PingState;
    type StateOperator = StateSaveOperator;
    type Message = PingMessage;
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for PingService {
    fn init(
        service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
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

        let mut inbound_relay = service_resources_handle.inbound_relay;
        let pong_outbound_relay = service_resources_handle
            .overwatch_handle
            .relay::<PongService>()
            .await?;

        let Self::State { mut pong_count } = initial_state;

        loop {
            tokio::select! {
                () = sleep(Duration::from_secs(1)) => {
                    println!("Sending Ping");
                    pong_outbound_relay.send(PongMessage::Ping).await.unwrap();
                }
                Some(message) = inbound_relay.recv() => {
                    match message {
                        PingMessage::Pong => {
                            pong_count += 1;
                            service_resources_handle.state_updater.update(
                                Some(Self::State { pong_count })
                            );
                            println!("Received Pong. Total: {pong_count}");
                        }
                    }
                }
                true = async {
                    pong_count >= 30
                } => {
                    println!("Received {pong_count} Pongs. Exiting...");
                    break;
                }
            }
        }

        let _ = service_resources_handle.overwatch_handle.shutdown().await;
        Ok(())
    }
}
