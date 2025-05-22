use std::any::Any;

use tokio::sync::mpsc as tokio_mpsc;

pub mod errors;
pub mod inbound;
pub mod outbound;

/// Message wrapper type.
pub type AnyMessage = Box<dyn Any + Send + 'static>;

pub(crate) type OutboundRelaySender<Message> = tokio_mpsc::Sender<Message>;
pub(crate) type InboundRelayReceiver<Message> = tokio_mpsc::Receiver<Message>;
pub(crate) fn relay_channel<Message>(
    buffer_size: usize,
) -> (OutboundRelaySender<Message>, InboundRelayReceiver<Message>) {
    tokio_mpsc::channel(buffer_size)
}

/// Channel to retrieve the receiver of the [`InboundRelay`].
/// The intended usage is oneshot-like, but having them as mpsc simplifies
/// reusing the relay when a service is stopped and started.
///
/// # Note
///
/// It's using synchronous mpsc channels because this is used in `Drop`.
pub mod inbound_relay_retriever {
    use std::sync::mpsc;

    use crate::services::relay::InboundRelayReceiver;

    pub type Sender<Message> = mpsc::Sender<InboundRelayReceiver<Message>>;
    pub type Receiver<Message> = mpsc::Receiver<InboundRelayReceiver<Message>>;
    #[must_use]
    pub fn channel<Message>() -> (Sender<Message>, Receiver<Message>) {
        mpsc::channel()
    }
}

pub use errors::{RelayError, ServiceError};
pub use inbound::InboundRelay;
pub use outbound::OutboundRelay;

pub struct Relay<Message> {
    pub inbound_relay: InboundRelay<Message>,
    pub outbound_relay: OutboundRelay<Message>,
    pub inbound_relay_retriever_sender: inbound_relay_retriever::Sender<Message>,
    pub inbound_relay_retriever_receiver: inbound_relay_retriever::Receiver<Message>,
}

impl<Message> Relay<Message> {
    // TODO: make buffer_size const?
    #[must_use]
    pub fn new(buffer_size: usize) -> Self {
        let (sender, receiver) = relay_channel(buffer_size);
        let (inbound_relay_retriever_sender, inbound_relay_retriever_receiver) =
            inbound_relay_retriever::channel();
        Self {
            inbound_relay: InboundRelay::new(
                receiver,
                inbound_relay_retriever_sender.clone(),
                buffer_size,
            ),
            outbound_relay: OutboundRelay::new(sender),
            inbound_relay_retriever_sender,
            inbound_relay_retriever_receiver,
        }
    }
}
