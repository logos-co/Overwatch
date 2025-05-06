use std::{
    any::Any,
    fmt::Debug,
    mem,
    pin::Pin,
    sync::mpsc as sync_mpsc,
    task::{Context, Poll},
};

use futures::{Sink, Stream};
use thiserror::Error;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio_util::sync::PollSender;
use tracing::error;

#[derive(Error, Debug)]
pub enum RelayError {
    #[error("couldn't relay message")]
    Send,
    #[error("relay is already connected")]
    AlreadyConnected,
    #[error("receiver failed due to {0:?}")]
    Receiver(Box<dyn Debug + Send + Sync>),
}

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("couldn't start service")]
    Start,
    #[error("couldn't shutdown service")]
    Shutdown,
}

/// Message wrapper type.
pub type AnyMessage = Box<dyn Any + Send + 'static>;

/// Channel to retrieve the consumer of the relay connection.
/// The intended usage is oneshot-like, but having them as mpsc simplifies
/// reusing the relay when a service is stopped and started.
pub type ConsumerSender<Message> = sync_mpsc::Sender<Receiver<Message>>;
pub type ConsumerReceiver<Message> = sync_mpsc::Receiver<Receiver<Message>>;

/// Channel receiver of a relay connection.
#[derive(Debug)]
pub struct InboundRelay<Message> {
    receiver: Receiver<Message>,
    /// Sender to return the consumer back to the caller
    /// This is used to maintain a single consumer while being able to reuse it
    /// when the same service is stopped and started.
    consumer_sender: ConsumerSender<Message>,
    /// Size of the relay buffer, used for consistency in a hack in Drop to
    /// return the receiver
    buffer_size: usize,
    _stats: (), // placeholder
}

impl<Message> InboundRelay<Message> {
    #[must_use]
    pub const fn new(
        receiver: Receiver<Message>,
        consumer_sender: ConsumerSender<Message>,
        buffer_size: usize,
    ) -> Self {
        Self {
            receiver,
            consumer_sender,
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
            consumer_sender,
            buffer_size,
            ..
        } = self;

        // Instantiate a fake receiver to swap with the original one
        // This is hack to take ownership of the receiver, required to send it back
        let (_sender, mut swapped_receiver) = channel(*buffer_size);
        mem::swap(&mut swapped_receiver, receiver);

        // Instantiate a fake return sender to swap with the original one
        // This is hack to take ownership of the receiver, required to call `send`
        let (mut swapped_consumer_sender, _oneshot_rx) = sync_mpsc::channel();
        mem::swap(&mut swapped_consumer_sender, consumer_sender);

        if let Err(e) = swapped_consumer_sender.send(swapped_receiver) {
            panic!("Failed returning receiver: {e:?}");
        }
    }
}

/// Channel sender of a relay connection.
pub struct OutboundRelay<Message> {
    sender: Sender<Message>,
    _stats: (), // placeholder
}

impl<Message> OutboundRelay<Message> {
    #[must_use]
    pub const fn new(sender: Sender<Message>) -> Self {
        Self { sender, _stats: () }
    }
}

impl<Message> Clone for OutboundRelay<Message> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _stats: (),
        }
    }
}

impl<Message> OutboundRelay<Message>
where
    Message: Send,
{
    /// Send a message to the relay connection
    ///
    /// # Errors
    ///
    /// If the message cannot be sent to the specified service.
    pub async fn send(&self, message: Message) -> Result<(), (RelayError, Message)> {
        self.sender
            .send(message)
            .await
            .map_err(|e| (RelayError::Send, e.0))
    }

    /// Send a message to the relay connection in a blocking fashion.
    ///
    /// The intended usage of this function is for sending data from
    /// synchronous code to asynchronous code.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution context.
    ///
    /// # Errors
    ///
    /// If the message cannot be sent to the specified service.
    pub fn blocking_send(&self, message: Message) -> Result<(), (RelayError, Message)> {
        self.sender
            .blocking_send(message)
            .map_err(|e| (RelayError::Send, e.0))
    }

    pub fn into_sink(self) -> impl Sink<Message> {
        PollSender::new(self.sender)
    }
}

pub struct Relay<Message> {
    pub inbound: InboundRelay<Message>,
    pub outbound: OutboundRelay<Message>,
    pub consumer_sender: ConsumerSender<Message>,
    pub consumer_receiver: ConsumerReceiver<Message>,
}

impl<Message> Relay<Message> {
    // TODO: make buffer_size const?
    #[must_use]
    pub fn new(buffer_size: usize) -> Self {
        let (sender, receiver) = channel(buffer_size);
        let (consumer_sender, consumer_receiver) = sync_mpsc::channel();
        Self {
            inbound: InboundRelay::new(receiver, consumer_sender.clone(), buffer_size),
            outbound: OutboundRelay::new(sender),
            consumer_sender,
            consumer_receiver,
        }
    }
}
