use std::{
    any::Any,
    fmt::Debug,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{Sink, Stream};
use thiserror::Error;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio_util::sync::PollSender;
use tracing::error;

#[derive(Error, Debug)]
pub enum RelayError<AggregatedServiceId> {
    #[error("error requesting relay to {to} service")]
    InvalidRequest { to: AggregatedServiceId },
    #[error("couldn't relay message")]
    Send,
    #[error("relay is already connected")]
    AlreadyConnected,
    #[error("service {service_id} is not available")]
    Unavailable { service_id: AggregatedServiceId },
    #[error("invalid message with type id [{type_id}] for service {service_id}")]
    InvalidMessage {
        type_id: String,
        service_id: &'static str,
    },
    #[error("receiver failed due to {0:?}")]
    Receiver(Box<dyn Debug + Send + Sync>),
}

/// Message wrapper type.
pub type AnyMessage = Box<dyn Any + Send + 'static>;

#[derive(Debug, Clone)]
pub struct NoMessage;

/// Result type when creating a relay connection.
pub type RelayResult<AggregatedServiceId> = Result<AnyMessage, RelayError<AggregatedServiceId>>;

/// Channel receiver of a relay connection.
#[derive(Debug)]
pub struct InboundRelay<Message> {
    receiver: Receiver<Message>,
    _stats: (), // placeholder
}

/// Channel sender of a relay connection.
pub struct OutboundRelay<Message, AggregatedServiceId> {
    sender: Sender<Message>,
    _stats: (), // placeholder
    _phantom: PhantomData<AggregatedServiceId>,
}

impl<Message, AggregatedServiceId> Clone for OutboundRelay<Message, AggregatedServiceId> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _stats: (),
            _phantom: PhantomData,
        }
    }
}

/// Relay channel builder.
// TODO: make buffer_size const?
#[must_use]
pub fn relay<Message, AggregatedServiceId>(
    buffer_size: usize,
) -> (
    InboundRelay<Message>,
    OutboundRelay<Message, AggregatedServiceId>,
) {
    let (sender, receiver) = channel(buffer_size);
    (
        InboundRelay {
            receiver,
            _stats: (),
        },
        OutboundRelay {
            sender,
            _stats: (),
            _phantom: PhantomData,
        },
    )
}

impl<Message> InboundRelay<Message> {
    /// Receive a message from the relay connections
    pub async fn recv(&mut self) -> Option<Message> {
        self.receiver.recv().await
    }
}

impl<Message, AggregatedServiceId> OutboundRelay<Message, AggregatedServiceId>
where
    Message: Send,
    AggregatedServiceId: Sync,
{
    /// Send a message to the relay connection
    ///
    /// # Errors
    ///
    /// If the message cannot be sent to the specified service.
    pub async fn send(
        &self,
        message: Message,
    ) -> Result<(), (RelayError<AggregatedServiceId>, Message)> {
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
    pub fn blocking_send(
        &self,
        message: Message,
    ) -> Result<(), (RelayError<AggregatedServiceId>, Message)> {
        self.sender
            .blocking_send(message)
            .map_err(|e| (RelayError::Send, e.0))
    }
}

impl<Message, AggregatedServiceId> OutboundRelay<Message, AggregatedServiceId>
where
    Message: Send,
{
    pub fn into_sink(self) -> impl Sink<Message> {
        PollSender::new(self.sender)
    }
}

impl<Message> Stream for InboundRelay<Message> {
    type Item = Message;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}
