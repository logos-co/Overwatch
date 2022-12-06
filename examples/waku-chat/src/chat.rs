use crate::network::*;
use async_trait::async_trait;
use overwatch_rs::services::handle::ServiceStateHandle;
use overwatch_rs::services::relay::{NoMessage, OutboundRelay};
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::channel;

/// Chat service handler
/// displays received messages, send new ones
pub struct ChatService {
    service_state: ServiceStateHandle<Self>,
}

#[derive(Deserialize, Serialize)]
struct Message {
    user: usize,
    msg: Box<[u8]>,
}

impl ServiceData for ChatService {
    const SERVICE_ID: ServiceId = "Chat";
    type Settings = usize;
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = NoMessage;
}

#[async_trait]
impl ServiceCore for ChatService {
    fn init(service_state: ServiceStateHandle<Self>) -> Result<Self, overwatch_rs::DynError> {
        Ok(Self { service_state })
    }

    async fn run(self) -> Result<(), overwatch_rs::DynError> {
        let Self {
            mut service_state, ..
        } = self;
        // TODO: waku should not end up in the public interface of the network service, at least not as a type
        let mut network_relay = service_state
            .overwatch_handle
            .relay::<NetworkService<waku::Waku>>()
            .connect()
            .await
            .unwrap();
        let user = service_state.settings_reader.get_updated_settings();
        let (sender, mut receiver) = channel(1);
        // TODO: typestate so I can't call send if it's not connected
        network_relay
            .send(NetworkMsg::Subscribe {
                kind: EventKind::Message,
                sender,
            })
            .await
            .unwrap();

        // send new messages
        // for interactive stdin I/O it's recommended to
        // use an external thread, see https://docs.rs/tokio/latest/tokio/io/struct.Stdin.html
        std::thread::spawn(move || loop {
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .expect("error reading message");
            input.truncate(input.trim().len());
            network_relay
                .blocking_send(NetworkMsg::Broadcast(
                    bincode::serialize(&Message {
                        user,
                        msg: input.as_bytes().to_vec().into_boxed_slice(),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                ))
                .unwrap();
            tracing::debug!("[sending]: {}...", input);
        });

        // print received messages
        while let Some(NetworkEvent::RawMessage(message)) = receiver.recv().await {
            if let Ok(msg) = bincode::deserialize::<Message>(&message) {
                if msg.user != user {
                    println!(
                        "[received][{}]: {}",
                        msg.user,
                        String::from_utf8_lossy(&msg.msg)
                    );
                }
            }
        }
        Ok(())
    }
}
