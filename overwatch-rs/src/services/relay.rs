// std
use std::any::Any;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
// crates
use futures::{poll, Sink, SinkExt, Stream};
use thiserror::Error;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::oneshot;
use tokio_util::sync::PollSender;
use tracing::{error, instrument};
// internal
use crate::overwatch::commands::{OverwatchCommand, RelayCommand, ReplyChannel};
use crate::overwatch::handle::OverwatchHandle;
use crate::services::{ServiceCore, ServiceId};

#[derive(Error, Debug)]
pub enum RelayError {
    #[error("error requesting relay to {to} service")]
    InvalidRequest { to: ServiceId },
    #[error("couldn't relay message")]
    Send,
    #[error("relay is already connected")]
    AlreadyConnected,
    #[error("service relay is disconnected")]
    Disconnected,
    #[error("service {service_id} is not available")]
    Unavailable { service_id: ServiceId },
    #[error("invalid message with type id [{type_id}] for service {service_id}")]
    InvalidMessage {
        type_id: String,
        service_id: &'static str,
    },
    #[error("receiver failed due to {0:?}")]
    Receiver(Box<dyn Debug + Send + Sync>),
}

/// Message wrapper type
pub type AnyMessage = Box<dyn Any + Send + 'static>;

#[derive(Debug, Clone)]
pub struct NoMessage;

impl RelayMessage for NoMessage {}

/// Result type when creating a relay connection
pub type RelayResult = Result<AnyMessage, RelayError>;

/// Marker type for relay messages
/// Notice that it is bound to 'static.
pub trait RelayMessage: 'static {}

/// Channel receiver of a relay connection
#[derive(Debug)]
pub struct InboundRelay<M> {
    receiver: Receiver<M>,
    _stats: (), // placeholder
}

/// Channel sender of a relay connection
pub struct OutboundRelay<M> {
    sender: Sender<M>,
    _stats: (), // placeholder
}

#[derive(Debug)]
pub struct Relay<S: ServiceCore> {
    _marker: PhantomData<S>,
    overwatch_handle: OverwatchHandle,
}

impl<S: ServiceCore> Clone for Relay<S> {
    fn clone(&self) -> Self {
        Self {
            _marker: PhantomData,
            overwatch_handle: self.overwatch_handle.clone(),
        }
    }
}

impl<M> Clone for OutboundRelay<M> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _stats: (),
        }
    }
}

// TODO: make buffer_size const?
/// Relay channel builder
pub fn relay<M>(buffer_size: usize) -> (InboundRelay<M>, OutboundRelay<M>) {
    let (sender, receiver) = channel(buffer_size);
    (
        InboundRelay {
            receiver,
            _stats: (),
        },
        OutboundRelay { sender, _stats: () },
    )
}

impl<M> InboundRelay<M> {
    /// Receive a message from the relay connections
    pub async fn recv(&mut self) -> Option<M> {
        self.receiver.recv().await
    }
}

impl<M: Send + 'static> OutboundRelay<M> {
    /// Send a message to the relay connection
    pub async fn send(&self, message: M) -> Result<(), (RelayError, M)> {
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
    /// This function panics if called within an asynchronous execution
    /// context.
    ///
    /// # Exa
    pub fn blocking_send(&self, message: M) -> Result<(), (RelayError, M)> {
        self.sender
            .blocking_send(message)
            .map_err(|e| (RelayError::Send, e.0))
    }

    pub fn into_sink(self) -> impl Sink<M> + Send + 'static {
        PollSender::new(self.sender)
    }
}

impl<S: ServiceCore> Relay<S> {
    pub fn new(overwatch_handle: OverwatchHandle) -> Self {
        Self {
            overwatch_handle,
            _marker: PhantomData,
        }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn connect(self) -> Result<OutboundRelay<S::Message>, RelayError> {
        let (reply, receiver) = oneshot::channel();
        self.request_relay(reply).await;
        self.handle_relay_response(receiver).await
    }

    async fn request_relay(&self, reply: oneshot::Sender<RelayResult>) {
        let relay_command = OverwatchCommand::Relay(RelayCommand {
            service_id: S::SERVICE_ID,
            reply_channel: ReplyChannel(reply),
        });
        self.overwatch_handle.send(relay_command).await;
    }

    #[instrument(skip_all, err(Debug))]
    async fn handle_relay_response(
        &self,
        receiver: oneshot::Receiver<RelayResult>,
    ) -> Result<OutboundRelay<S::Message>, RelayError> {
        let response = receiver.await;
        match response {
            Ok(Ok(message)) => match message.downcast::<OutboundRelay<S::Message>>() {
                Ok(channel) => Ok(*channel),
                Err(m) => Err(RelayError::InvalidMessage {
                    type_id: format!("{:?}", m.type_id()),
                    service_id: S::SERVICE_ID,
                }),
            },
            Ok(Err(e)) => Err(e),
            Err(e) => Err(RelayError::Receiver(Box::new(e))),
        }
    }
}

impl<M> Stream for InboundRelay<M> {
    type Item = M;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}
