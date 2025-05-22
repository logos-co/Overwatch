use std::{
    mem,
    pin::Pin,
    sync::mpsc as std_mpsc,
    task::{Context, Poll},
};

use futures::Stream;
use tokio::sync::mpsc::{channel, Receiver};
use tracing::error;

use crate::services::relay::InboundRelaySender;

/// Channel receiver of a relay connection.
#[derive(Debug)]
pub struct InboundRelay<Message> {
    receiver: Receiver<Message>,
    /// Sender to return the receiver to the caller
    /// This is used to maintain a single [`InboundRelay`] throughout the
    /// lifetime of a `Service`, so other services can maintain their relay.
    inbound_relay_sender: InboundRelaySender<Message>,
    /// Size of the relay buffer, used for consistency in a hack in Drop to
    /// return the receiver
    buffer_size: usize,
    _stats: (), // placeholder
}

impl<Message> InboundRelay<Message> {
    #[must_use]
    pub const fn new(
        receiver: Receiver<Message>,
        inbound_relay_sender: InboundRelaySender<Message>,
        buffer_size: usize,
    ) -> Self {
        Self {
            receiver,
            inbound_relay_sender,
            buffer_size,
            _stats: (),
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
            inbound_relay_sender,
            buffer_size,
            ..
        } = self;

        // Instantiate a fake receiver to swap with the original one
        // This is a hack to take ownership of the receiver, required to send it back
        let (_sender, mut swapped_receiver) = channel(*buffer_size);
        mem::swap(&mut swapped_receiver, receiver);

        // Instantiate a fake return sender to swap with the original one
        // This is a hack to take ownership of the sender, required to call `send`
        let (mut swapped_inbound_relay, _oneshot_rx) = std_mpsc::channel();
        mem::swap(&mut swapped_inbound_relay, inbound_relay_sender);

        if let Err(error) = swapped_inbound_relay.send(swapped_receiver) {
            error!("Failed returning receiver: {error}. This is expected if the `ServiceRunner` has been killed.");
        }
    }
}
