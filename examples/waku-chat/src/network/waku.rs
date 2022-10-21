use super::*;
use ::waku::*;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tokio::sync::mpsc::Sender;

pub struct Waku {
    waku: WakuNodeHandle<Running>,
    subscribers: Arc<RwLock<Vec<Sender<NetworkEvent>>>>,
}

impl NetworkBackend for Waku {
    fn new(config: NetworkConfig) -> Self {
        let mut waku_config = WakuNodeConfig::default();
        waku_config.port = Some(config.port as usize);
        let waku = waku_new(Some(waku_config)).unwrap().start().unwrap();
        for peer in config.peers {
            let addr = Multiaddr::from_str(&peer).unwrap();
            let peer_id = waku.add_peer(&addr, waku::ProtocolId::Relay).unwrap();
            waku.connect_peer_with_id(peer_id, None).unwrap();
        }
        waku.relay_subscribe(None).unwrap();
        assert!(waku.relay_enough_peers(None).unwrap());
        tracing::info!("waku listening on {}", waku.listen_addresses().unwrap()[0]);
        Self {
            waku,
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn subscribe(&mut self, sender: Sender<NetworkEvent>) {
        self.subscribers.write().unwrap().push(sender);
        tracing::debug!("someone subscribed");
        let subscribers = Arc::clone(&self.subscribers);
        waku_set_event_callback(move |sig| {
            match sig.event() {
                Event::WakuMessage(ref message_event) => {
                    tracing::debug!("received message event");
                    // we can probably avoid sending a copy to each subscriber and just borrow / clone on demand
                    for s in subscribers.read().unwrap().iter() {
                        s.try_send(NetworkEvent::RawMessage(
                            message_event
                                .waku_message()
                                .payload()
                                .to_vec()
                                .into_boxed_slice(),
                        ))
                        .unwrap()
                    }
                }
                _ => tracing::debug!("unsupported event"),
            }
        });
    }

    fn broadcast(&self, msg: Box<[u8]>) {
        let content_topic = WakuContentTopic::from_str("/waku/2/default-waku/proto").unwrap();
        let message = WakuMessage::new(
            msg,
            content_topic,
            1,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as usize,
        );
        let msg_id = self
            .waku
            .relay_publish_message(&message, None, None)
            .unwrap();
        tracing::debug!("sent msg {:?} with id {}", message.payload(), msg_id);
    }
}
