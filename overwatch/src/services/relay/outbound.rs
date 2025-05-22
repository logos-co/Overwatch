use futures::Sink;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::PollSender;

use crate::services::relay::errors::RelayError;

/// Channel sender of a relay connection.
pub struct OutboundRelay<Message> {
    sender: Sender<Message>,
}

impl<Message> OutboundRelay<Message> {
    #[must_use]
    pub const fn new(sender: Sender<Message>) -> Self {
        Self { sender }
    }
}

impl<Message> Clone for OutboundRelay<Message> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
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
