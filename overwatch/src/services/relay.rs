use futures::{Sink, Stream};
use std::sync::Arc;
use std::{
    any::Any,
    fmt::Debug,
    mem,
    pin::Pin,
    task::{Context, Poll},
};
use thiserror::Error;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::oneshot;
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
}

/// Message wrapper type.
pub type AnyMessage = Box<dyn Any + Send + 'static>;

/// Result type when creating a relay connection.
pub type RelayResult = Result<AnyMessage, RelayError>;

/// Channel receiver of a relay connection.
#[derive(Debug)]
pub struct InboundRelay<Message> {
    receiver: Receiver<Message>,
    /// Sender to return the consumer back to the caller
    /// This is used to maintain a single consumer while being able to reuse it when the same
    /// service is stopped and started.
    return_sender: oneshot::Sender<Receiver<Message>>,
    _stats: (), // placeholder
}

impl<Message> InboundRelay<Message> {
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
            return_sender,
            ..
        } = self;

        // Instantiate a fake receiver to swap with the original one
        // This is hack to take ownership of the receiver, required to send it back
        let (_sender, mut swapped_receiver) = channel(32);
        mem::swap(&mut swapped_receiver, receiver);

        // Instantiate a fake return sender to swap with the original one
        // This is hack to take ownership of the receiver, required to call `send`
        let (mut swapped_return_sender, _oneshot_rx) = oneshot::channel();
        mem::swap(&mut swapped_return_sender, return_sender);

        if let Err(e) = swapped_return_sender.send(swapped_receiver) {
            panic!("Failed returning receiver: {:?}", e);
        }
    }
}

/// Channel sender of a relay connection.
pub struct OutboundRelay<Message> {
    sender: Sender<Message>,
    /// Receiver to fetch the consumer
    /// This is used to maintain a single consumer while being able to reuse it when the same
    /// service is stopped and started.
    /// TODO: The relay needs to be recreated when the service is stopped and started, to reset
    ///   the channel.
    return_receiver: Arc<oneshot::Receiver<Receiver<Message>>>,
    _stats: (), // placeholder
}

impl<Message> Clone for OutboundRelay<Message> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            return_receiver: Arc::clone(&self.return_receiver),
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

/// Relay channel builder.
// TODO: make buffer_size const?
#[must_use]
pub fn relay<Message>(buffer_size: usize) -> (InboundRelay<Message>, OutboundRelay<Message>) {
    let (sender, receiver) = channel(buffer_size);
    let (return_sender, return_receiver) = oneshot::channel();
    let return_receiver = Arc::new(return_receiver);
    (
        InboundRelay {
            receiver,
            return_sender,
            _stats: (),
        },
        OutboundRelay {
            sender,
            return_receiver,
            _stats: (),
        },
    )
}
