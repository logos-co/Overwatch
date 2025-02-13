// Crates
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use overwatch_rs::{DynError, ServiceStateHandle};
use std::time::Duration;
use tokio::time::sleep;
// Internal
use crate::messages::{PingMessage, PongMessage};
use crate::operators::StateSaveOperator;
use crate::service_pong::PongService;
use crate::settings::PingSettings;
use crate::states::PingState;

pub struct PingService {
    service_state_handle: ServiceStateHandle<Self>,
    initial_state: <Self as ServiceData>::State,
}

impl ServiceData for PingService {
    const SERVICE_ID: ServiceId = "ping";
    type Settings = PingSettings;
    type State = PingState;
    type StateOperator = StateSaveOperator;
    type Message = PingMessage;
}

#[async_trait::async_trait]
impl ServiceCore for PingService {
    fn init(
        service_state_handle: ServiceStateHandle<Self>,
        initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self {
            service_state_handle,
            initial_state,
        })
    }

    async fn run(self) -> Result<(), DynError> {
        let Self {
            service_state_handle,
            initial_state,
        } = self;

        let mut inbound_relay = service_state_handle.inbound_relay;
        let pong_outbound_relay = service_state_handle
            .overwatch_handle
            .relay::<PongService>()
            .connect()
            .await?;

        let Self::State { mut pong_count } = initial_state;

        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(1)) => {
                    println!("Sending Ping");
                    pong_outbound_relay.send(PongMessage::Ping).await.unwrap();
                }
                Some(message) = inbound_relay.recv() => {
                    match message {
                        PingMessage::Pong => {
                            pong_count += 1;
                            service_state_handle.state_updater.update(
                                Self::State { pong_count }
                            );
                            println!("Received Pong. Total: {}", pong_count);
                        }
                    }
                }
                true = async {
                    pong_count >= 30
                } => {
                    println!("Received {} Pongs. Exiting...", pong_count);
                    break;
                }
            }
        }

        service_state_handle.overwatch_handle.shutdown().await;
        Ok(())
    }
}
