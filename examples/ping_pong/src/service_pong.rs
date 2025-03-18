use overwatch::{
    services::{
        state::{NoOperator, NoState},
        ServiceCore, ServiceData, ServiceId,
    },
    DynError, OpaqueServiceStateHandle,
};

use crate::{
    messages::{PingMessage, PongMessage},
    service_ping::PingService,
    AggregatedServiceId,
};

pub struct PongService {
    service_state_handle: OpaqueServiceStateHandle<Self, AggregatedServiceId>,
}

impl ServiceData for PongService {
    const SERVICE_ID: ServiceId = "pong";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = PongMessage;
}

#[async_trait::async_trait]
impl ServiceCore<AggregatedServiceId> for PongService {
    fn init(
        service_state_handle: OpaqueServiceStateHandle<Self, AggregatedServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
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
