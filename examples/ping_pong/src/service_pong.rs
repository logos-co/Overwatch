use overwatch::{
    services::{
        state::{NoOperator, NoState},
        ServiceCore, ServiceData,
    },
    DynError, OpaqueServiceResourcesHandle,
};

use crate::{
    messages::{PingMessage, PongMessage},
    service_ping::PingService,
    RuntimeServiceId,
};

pub struct PongService {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

impl ServiceData for PongService {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = PongMessage;
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for PongService {
    fn init(
        service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self {
            service_resources_handle,
        })
    }

    async fn run(self) -> Result<(), DynError> {
        let Self {
            service_resources_handle,
        } = self;

        let mut inbound_relay = service_resources_handle.inbound_relay;
        let ping_outbound_relay = service_resources_handle
            .overwatch_handle
            .relay::<PingService>()
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
