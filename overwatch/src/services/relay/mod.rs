use std::{any::Any, sync::mpsc as std_mpsc};

use tokio::sync::mpsc as tokio_mpsc;

pub mod errors;
pub mod inbound;
pub mod outbound;

/// Message wrapper type.
pub type AnyMessage = Box<dyn Any + Send + 'static>;

/// Channel to retrieve the receiver of the [`InboundRelay`].
/// The intended usage is oneshot-like, but having them as mpsc simplifies
/// reusing the relay when a service is stopped and started.
// TODO: Update this name to something more meaningful. E.g.:
//  InboundConsumerSender
// TODO: Try async
pub type InboundRelaySender<Message> = std_mpsc::Sender<tokio_mpsc::Receiver<Message>>;
pub type InboundRelayReceiver<Message> = std_mpsc::Receiver<tokio_mpsc::Receiver<Message>>;

pub use errors::{RelayError, ServiceError};
pub use inbound::InboundRelay;
pub use outbound::OutboundRelay;

pub struct Relay<Message> {
    pub inbound_relay: InboundRelay<Message>,
    pub outbound_relay: OutboundRelay<Message>,
    pub inbound_relay_sender: InboundRelaySender<Message>,
    pub inbound_relay_receiver: InboundRelayReceiver<Message>,
}

impl<Message> Relay<Message> {
    // TODO: make buffer_size const?
    #[must_use]
    pub fn new(buffer_size: usize) -> Self {
        let (sender, receiver) = tokio_mpsc::channel(buffer_size);
        let (inbound_relay_sender, inbound_relay_receiver) = std_mpsc::channel();
        Self {
            inbound_relay: InboundRelay::new(receiver, inbound_relay_sender.clone(), buffer_size),
            outbound_relay: OutboundRelay::new(sender),
            inbound_relay_sender,
            inbound_relay_receiver,
        }
    }
}
