use std::time::Duration;

use futures::StreamExt as _;
use overwatch::{
    services::{life_cycle::LifecycleMessage, AsServiceId, ServiceCore, ServiceData},
    DynError, OpaqueServiceStateHandle,
};
use tokio::time::sleep;

use crate::{
    messages::{PingMessage, PongMessage},
    operators::StateSaveOperator,
    service_pong::PongService,
    settings::PingSettings,
    states::PingState,
    RuntimeServiceId,
};

pub struct PingService {
    service_state_handle: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
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
        service_state_handle: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
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

        let mut lifecycle_stream = service_state_handle.lifecycle_handle.message_stream();

        let lifecycle_message = lifecycle_stream
            .next()
            .await
            .expect("first received message to be a lifecycle message.");

        let sender = match lifecycle_message {
            LifecycleMessage::Shutdown(sender) => {
                println!("Service started 1.");
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
        let pong_outbound_relay = service_state_handle
            .overwatch_handle
            .relay::<PongService>()
            .await?;
        let Self::State { mut pong_count } = initial_state;

        if sender.send(()).is_err() {
            eprintln!(
                "Error sending successful startup signal from service {}",
                <RuntimeServiceId as AsServiceId<Self>>::SERVICE_ID
            );
        }

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
                            service_state_handle.state_updater.update(
                                Self::State { pong_count }
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

        service_state_handle.overwatch_handle.shutdown().await;
        Ok(())
    }
}
