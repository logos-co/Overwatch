// Crates
use crate::messages::{PingMessage, PongMessage};
use crate::service_ping::PingService;
use overwatch_rs::services::handle::ServiceStateHandle;
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use overwatch_rs::DynError;

pub struct PongService {
    service_state_handle: ServiceStateHandle<Self>,
}

impl ServiceData for PongService {
    const SERVICE_ID: ServiceId = "";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = PongMessage;
}

#[async_trait::async_trait]
impl ServiceCore for PongService {
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
        let ping_outbound_relay = service_state_handle
            .overwatch_handle
            .relay::<PingService>()
            .connect()
            .await?;

        while let Some(message) = inbound_relay.recv().await {
            match message {
                PongMessage::Ping => {
                    println!("Received Ping. Sending Pong.");
                    ping_outbound_relay.send(PingMessage::Pong).await.unwrap();
                }
            }
        }
        Ok(())
    }
}
