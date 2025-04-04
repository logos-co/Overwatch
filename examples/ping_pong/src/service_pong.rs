use futures::StreamExt as _;
use overwatch::{
    services::{
        life_cycle::LifecycleMessage,
        state::{NoOperator, NoState},
        AsServiceId, ServiceCore, ServiceData,
    },
    DynError, OpaqueServiceStateHandle,
};

use crate::{
    messages::{PingMessage, PongMessage},
    service_ping::PingService,
    RuntimeServiceId,
};

pub struct PongService {
    service_state_handle: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
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
        service_state_handle: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
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

        let mut lifecycle_stream = service_state_handle.lifecycle_handle.message_stream();

        let lifecycle_message = lifecycle_stream
            .next()
            .await
            .expect("first received message to be a lifecycle message.");

        let sender = match lifecycle_message {
            LifecycleMessage::Shutdown(sender) => {
                if sender.send(()).is_err() {
                    eprintln!(
                        "Error sending successful shutdown signal from service {}",
                        <RuntimeServiceId as AsServiceId<Self>>::SERVICE_ID
                    );
                }
                return Ok(());
            }
            LifecycleMessage::Kill => return Ok(()),
            // Continue below if a `Start` message is received.
            LifecycleMessage::Start(sender) => sender,
        };

        let mut inbound_relay = service_state_handle.inbound_relay;
        let ping_outbound_relay = service_state_handle
            .overwatch_handle
            .relay::<PingService>()
            .await?;

        if sender.send(()).is_err() {
            eprintln!(
                "Error sending successful startup signal from service {}",
                <RuntimeServiceId as AsServiceId<Self>>::SERVICE_ID
            );
        }

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
