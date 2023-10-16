// std
use std::any::Any;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
// crates
use futures::{Sink, Stream};
use thiserror::Error;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::{broadcast, oneshot};
use tokio_util::sync::PollSender;
use tracing::{error, instrument};
// internal
use crate::overwatch::commands::{OverwatchCommand, RelayCommand, ReplyChannel};
use crate::overwatch::handle::OverwatchHandle;
use crate::services::relay::RelayState::{Pending, Ready};
use crate::services::{ServiceData, ServiceId};

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
pub struct Relay<S> {
    overwatch_handle: OverwatchHandle,
    _bound: PhantomBound<S>,
}

impl<T> Clone for Relay<T> {
    fn clone(&self) -> Self {
        Self {
            overwatch_handle: self.overwatch_handle.clone(),
            _bound: PhantomBound {
                _inner: PhantomData,
            },
        }
    }
}

// Like PhantomData<T> but without
// ownership of T
#[derive(Debug)]
struct PhantomBound<T> {
    _inner: PhantomData<*const T>,
}

unsafe impl<T> Send for PhantomBound<T> {}
unsafe impl<T> Sync for PhantomBound<T> {}

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
fn relay<M>(buffer_size: usize) -> (InboundRelay<M>, OutboundRelay<M>) {
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

impl<M> OutboundRelay<M> {
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
}

impl<M: Send + 'static> OutboundRelay<M> {
    pub fn into_sink(self) -> impl Sink<M> {
        PollSender::new(self.sender)
    }
}

impl<S: ServiceData> Relay<S> {
    pub fn new(overwatch_handle: OverwatchHandle) -> Self {
        Self {
            overwatch_handle,
            _bound: PhantomBound {
                _inner: PhantomData,
            },
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
            Ok(Ok(message)) => match message.downcast::<OutboundRelayState<S::Message>>() {
                Ok(channel) => channel.connect().await.inner_relay(),
                Err(m) => Err(RelayError::InvalidMessage {
                    type_id: format!("{:?}", (*m).type_id()),
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

pub enum RelayState<C, R> {
    Pending(C),
    Ready(R),
}

pub type InboundRelayState<M> = RelayState<broadcast::Sender<OutboundRelay<M>>, InboundRelay<M>>;

pub type OutboundRelayState<M> =
    RelayState<broadcast::Receiver<OutboundRelay<M>>, OutboundRelay<M>>;

impl<M> Clone for RelayState<broadcast::Receiver<OutboundRelay<M>>, OutboundRelay<M>> {
    fn clone(&self) -> Self {
        match self {
            RelayState::Pending(receiver) => Pending(receiver.resubscribe()),
            RelayState::Ready(receiver) => Ready(receiver.clone()),
        }
    }
}
impl<M> RelayState<broadcast::Receiver<OutboundRelay<M>>, OutboundRelay<M>> {
    #[must_use]
    pub(crate) async fn connect(self) -> Self {
        use RelayState::*;
        match self {
            Pending(mut c) => {
                let relay = c
                    .recv()
                    .await
                    .expect("An outbound relay should be available");
                Ready(relay)
            }
            Ready(relay) => Ready(relay),
        }
    }

    pub fn inner_relay(&self) -> Result<OutboundRelay<M>, RelayError> {
        match self {
            RelayState::Pending(_) => Err(RelayError::Disconnected),
            RelayState::Ready(relay) => Ok(relay.clone()),
        }
    }
}

impl<M> RelayState<broadcast::Sender<OutboundRelay<M>>, InboundRelay<M>> {
    #[must_use]
    pub(crate) fn connect(self, buffer_size: usize) -> Self {
        use RelayState::*;
        let (inbound, outbound) = relay(buffer_size);
        match self {
            Pending(c) => {
                c.send(outbound)
                    .unwrap_or_else(|_| panic!("An outbound relay should be available"));
                Ready(inbound)
            }
            Ready(relay) => Ready(relay),
        }
    }

    pub fn inner_relay(self) -> InboundRelay<M> {
        match self {
            RelayState::Pending(_) => {
                panic!("Relay wasnt connected");
            }
            RelayState::Ready(relay) => relay,
        }
    }
}

pub(crate) fn relay_state<M>() -> (InboundRelayState<M>, OutboundRelayState<M>) {
    let (sender, receiver) = broadcast::channel(1);
    (
        InboundRelayState::Pending(sender),
        OutboundRelayState::Pending(receiver),
    )
}
