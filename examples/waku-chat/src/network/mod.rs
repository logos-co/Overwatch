pub mod waku;
use async_trait::async_trait;
use overwatch_rs::services::handle::ServiceStateHandle;
use overwatch_rs::services::relay::RelayMessage;
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use std::fmt::Debug;
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub enum NetworkMsg {
    Broadcast(Box<[u8]>),
    Subscribe {
        kind: EventKind,
        sender: Sender<NetworkEvent>,
    },
}

impl RelayMessage for NetworkMsg {}

#[derive(Debug)]
pub enum EventKind {
    Message,
}

#[derive(Debug)]
pub enum NetworkEvent {
    RawMessage(Box<[u8]>),
}

#[derive(Clone, Debug)]
pub struct NetworkConfig {
    pub port: u16,
    pub peers: Vec<String>,
}

pub struct NetworkService<I: NetworkBackend + Send + 'static> {
    implem: I,
    service_state: ServiceStateHandle<Self>,
}

impl<I: NetworkBackend + Send + 'static> ServiceData for NetworkService<I> {
    const SERVICE_ID: ServiceId = "Network";
    type Settings = NetworkConfig;
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = NetworkMsg;
}

#[async_trait]
impl<I: NetworkBackend + Send + 'static> ServiceCore for NetworkService<I> {
    fn init(mut service_state: ServiceStateHandle<Self>) -> Result<Self, overwatch_rs::DynError> {
        Ok(Self {
            implem: <I as NetworkBackend>::new(
                service_state.settings_reader.get_updated_settings(),
            ),
            service_state,
        })
    }

    async fn run(mut self) -> Result<(), overwatch_rs::DynError> {
        let Self {
            service_state,
            mut implem,
        } = self;
        let mut relay = service_state.inbound_relay;

        while let Some(msg) = relay.recv().await {
            match msg {
                NetworkMsg::Broadcast(msg) => implem.broadcast(msg),
                NetworkMsg::Subscribe { kind: _, sender } => implem.subscribe(sender),
            }
        }
        Ok(())
    }
}

pub trait NetworkBackend {
    fn new(config: NetworkConfig) -> Self;
    fn broadcast(&self, msg: Box<[u8]>);
    fn subscribe(&mut self, sender: Sender<NetworkEvent>);
}
