use std::{
    any::Any,
    fmt::Debug,
    pin::Pin,
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

/// Message wrapper type.
pub type AnyMessage = Box<dyn Any + Send + 'static>;

/// Result type when creating a relay connection.
pub type RelayResult = Result<AnyMessage, RelayError>;

/// Channel receiver of a relay connection.
#[derive(Debug)]
pub struct InboundRelay<Message> {
    receiver: Receiver<Message>,
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

/// Channel sender of a relay connection.
pub struct OutboundRelay<Message> {
    sender: Sender<Message>,
    _stats: (), // placeholder
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

/// Relay channel builder.
// TODO: make buffer_size const?
#[must_use]
pub fn relay<Message>(buffer_size: usize) -> (InboundRelay<Message>, OutboundRelay<Message>) {
    let (sender, receiver) = channel(buffer_size);
    (
        InboundRelay {
            receiver,
            _stats: (),
        },
        OutboundRelay { sender, _stats: () },
    )
}
