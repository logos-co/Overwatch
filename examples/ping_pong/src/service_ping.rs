// Crates
use overwatch_rs::services::handle::ServiceStateHandle;
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use overwatch_rs::DynError;
use std::time::Duration;
use tokio::time::sleep;
// Internal
use crate::messages::{PingMessage, PongMessage};
use crate::service_pong::PongService;

pub struct PingService {
    service_state_handle: ServiceStateHandle<Self>,
}

impl ServiceData for PingService {
    const SERVICE_ID: ServiceId = "ping";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = PingMessage;
}

#[async_trait::async_trait]
impl ServiceCore for PingService {
    fn init(service_state_handle: ServiceStateHandle<Self>) -> Result<Self, DynError> {
        Ok(Self {
            service_state_handle,
        })
    }

    async fn run(self) -> Result<(), DynError> {
        let Self {
            service_state_handle,
        } = self;

        let mut inbound_relay = service_state_handle.inbound_relay;
        let pong_outbound_relay = service_state_handle
            .overwatch_handle
            .relay::<PongService>()
            .connect()
            .await?;

        let mut pong_count = 0;

        loop {
            tokio::select! {
                 _ = sleep(Duration::from_secs(1)) => {
                     println!("Sending Ping");
                     pong_outbound_relay.send(PongMessage::Ping).await.unwrap();
                 }
                 Some(message) = inbound_relay.recv() => {
                     match message {
                         PingMessage::Pong => {
                             println!("Received Pong");
                             pong_count += 1;
                         }
                     }
                 }
                 true = async {
                     pong_count >= 5
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
