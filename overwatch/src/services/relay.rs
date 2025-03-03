use std::{
    any::Any,
    fmt::Debug,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{Sink, Stream};
use thiserror::Error;
use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    oneshot,
};
use tokio_util::sync::PollSender;
use tracing::error;
#[cfg(feature = "instrumentation")]
use tracing::instrument;

use crate::{
    overwatch::{
        commands::{OverwatchCommand, RelayCommand, ReplyChannel},
        handle::OverwatchHandle,
    },
    services::{ServiceData, ServiceId},
};

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

/// Message wrapper type.
pub type AnyMessage = Box<dyn Any + Send + 'static>;

#[derive(Debug, Clone)]
pub struct NoMessage;

impl RelayMessage for NoMessage {}

/// Result type when creating a relay connection.
pub type RelayResult = Result<AnyMessage, RelayError>;

/// Marker type for relay messages.
///
/// Note that it is bound to 'static.
pub trait RelayMessage: 'static {}

/// Channel receiver of a relay connection.
#[derive(Debug)]
pub struct InboundRelay<Message> {
    receiver: Receiver<Message>,
    _stats: (), // placeholder
}

/// Channel sender of a relay connection.
pub struct OutboundRelay<Message> {
    sender: Sender<Message>,
    _stats: (), // placeholder
}

#[derive(Debug)]
pub struct Relay<Service> {
    overwatch_handle: OverwatchHandle,
    _bound: PhantomBound<Service>,
}

impl<Service> Clone for Relay<Service> {
    fn clone(&self) -> Self {
        Self {
            overwatch_handle: self.overwatch_handle.clone(),
            _bound: PhantomBound {
                _inner: PhantomData,
            },
        }
    }
}

// Like PhantomData<T> but without ownership of T
#[derive(Debug)]
struct PhantomBound<T> {
    _inner: PhantomData<*const T>,
}

unsafe impl<T> Send for PhantomBound<T> {}
unsafe impl<T> Sync for PhantomBound<T> {}

impl<Message> Clone for OutboundRelay<Message> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _stats: (),
        }
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

impl<Message> InboundRelay<Message> {
    /// Receive a message from the relay connections
    pub async fn recv(&mut self) -> Option<Message> {
        self.receiver.recv().await
    }
}

impl<Message> OutboundRelay<Message> {
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
}

impl<Message> OutboundRelay<Message>
where
    Message: Send,
{
    pub fn into_sink(self) -> impl Sink<Message> {
        PollSender::new(self.sender)
    }
}

impl<Service> Relay<Service>
where
    Service: ServiceData,
    Service::Message: 'static,
{
    #[must_use]
    pub const fn new(overwatch_handle: OverwatchHandle) -> Self {
        Self {
            overwatch_handle,
            _bound: PhantomBound {
                _inner: PhantomData,
            },
        }
    }

    #[cfg_attr(feature = "instrumentation", instrument(skip(self), err(Debug)))]
    pub async fn connect(self) -> Result<OutboundRelay<Service::Message>, RelayError> {
        let (reply, receiver) = oneshot::channel();
        self.request_relay(reply).await;
        self.handle_relay_response(receiver).await
    }

    async fn request_relay(&self, reply: oneshot::Sender<RelayResult>) {
        let relay_command = OverwatchCommand::Relay(RelayCommand {
            service_id: Service::SERVICE_ID,
            reply_channel: ReplyChannel(reply),
        });
        self.overwatch_handle.send(relay_command).await;
    }

    #[cfg_attr(feature = "instrumentation", instrument(skip_all, err(Debug)))]
    async fn handle_relay_response(
        &self,
        receiver: oneshot::Receiver<RelayResult>,
    ) -> Result<OutboundRelay<Service::Message>, RelayError> {
        let response = receiver.await;
        match response {
            Ok(Ok(message)) => match message.downcast::<OutboundRelay<Service::Message>>() {
                Ok(channel) => Ok(*channel),
                Err(m) => Err(RelayError::InvalidMessage {
                    type_id: format!("{:?}", (*m).type_id()),
                    service_id: Service::SERVICE_ID,
                }),
            },
            Ok(Err(e)) => Err(e),
            Err(e) => Err(RelayError::Receiver(Box::new(e))),
        }
    }
}

impl<Message> Stream for InboundRelay<Message> {
    type Item = Message;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}
