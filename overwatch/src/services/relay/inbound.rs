use std::{
    mem,
    pin::Pin,
    task::{Context, Poll},
};

use futures::Stream;
use tracing::error;

use crate::services::relay::{inbound_relay_retriever, relay_channel, InboundRelayReceiver};

/// Channel receiver of a relay connection.
#[derive(Debug)]
pub struct InboundRelay<Message> {
    receiver: InboundRelayReceiver<Message>,
    /// Sender to return the receiver to the caller
    /// This is used to maintain a single [`InboundRelay`] throughout the
    /// lifetime of a `Service`, so other services can maintain their relay.
    retriever_sender: inbound_relay_retriever::Sender<Message>,
    /// Size of the relay buffer, used for consistency in a hack in Drop to
    /// return the receiver
    buffer_size: usize,
}

impl<Message> InboundRelay<Message> {
    #[must_use]
    pub const fn new(
        receiver: InboundRelayReceiver<Message>,
        retriever_sender: inbound_relay_retriever::Sender<Message>,
        buffer_size: usize,
    ) -> Self {
        Self {
            receiver,
            retriever_sender,
            buffer_size,
        }
    }

    /// Receive a message from the relay connections
    pub async fn recv(&mut self) -> Option<Message> {
        self.receiver.recv().await
    }
}

impl<Message> Stream for InboundRelay<Message> {
    type Item = Message;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

impl<Message> Drop for InboundRelay<Message> {
    fn drop(&mut self) {
        let Self {
            receiver,
            retriever_sender,
            buffer_size,
            ..
        } = self;

        // Instantiate a fake receiver to swap with the original one
        // This is a hack to take ownership of the receiver, required to send it back
        let (_sender, mut swapped_receiver) = relay_channel(*buffer_size);
        mem::swap(&mut swapped_receiver, receiver);

        // Instantiate a fake return sender to swap with the original one
        // This is a hack to take ownership of the sender, required to call `send`
        let (mut swapped_retriever_sender, _oneshot_rx) = inbound_relay_retriever::channel();
        mem::swap(&mut swapped_retriever_sender, retriever_sender);

        if let Err(error) = swapped_retriever_sender.send(swapped_receiver) {
            error!("Failed returning receiver: {error}. This is expected if the `ServiceRunner` has been killed.");
        }
    }
}
