use std::{
    default::Default,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{Stream, StreamExt};
use tokio::sync::mpsc::{channel, Sender};
use tokio_stream::wrappers::ReceiverStream;

use crate::{utils::finished_signals, DynError};

/// Message type for `Service` lifecycle events.
#[derive(Debug)]
pub enum LifecycleMessage {
    /// Starts the `Service`.
    ///
    /// If the `Service` has been stopped with [`LifecycleMessage::Stop`], it
    /// will be restarted.
    ///
    /// # Arguments
    ///
    /// - [`finished_signals::Sender`]: A [`finished_signals::Signal`] will be
    ///   sent through the associated channel upon completion of the task.
    Start(finished_signals::Sender),

    /// Stops the `Service`.
    ///
    /// Inner `Service` operations are not guaranteed to be completed.
    /// Despite that, `Service`s stopped this way can be restarted (from a
    /// previously saved point or from the default initial state) by sending
    /// a [`LifecycleMessage::Start`].
    ///
    /// # Arguments
    ///
    /// - [`finished_signals::Sender`]: A [`finished_signals::Signal`] will be
    ///   sent through the associated channel upon completion of the task.
    Stop(finished_signals::Sender),
}

#[derive(Clone)]
pub struct LifecycleNotifier {
    sender: Sender<LifecycleMessage>,
}

impl LifecycleNotifier {
    #[must_use]
    pub const fn new(sender: Sender<LifecycleMessage>) -> Self {
        Self { sender }
    }

    /// Send a [`LifecycleMessage`] to the `Service`.
    ///
    /// # Errors
    ///
    /// If the message cannot be sent to the service.
    pub async fn send(&self, msg: LifecycleMessage) -> Result<(), DynError> {
        self.sender
            .send(msg)
            .await
            .map_err(|e| Box::new(e) as DynError)
    }
}

/// Handle for lifecycle communications with a `Service`.
pub struct LifecycleHandle {
    stream: ReceiverStream<LifecycleMessage>,
    notifier: LifecycleNotifier,
}

/// A handle to manage [`LifecycleMessage`]s for a `Service`.
///
/// All lifecycle computations are processed sequentially to prevent race
/// conditions (e.g.: unordered messages).
///
/// [`LifecycleMessage`] senders wait until the channel is empty before sending
/// a new message, akin to a mutex.
impl LifecycleHandle {
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = channel(1);
        Self {
            stream: ReceiverStream::new(receiver),
            notifier: LifecycleNotifier::new(sender),
        }
    }

    /// Returns the internal [`LifecycleNotifier`] to the `Service`.
    ///
    /// This can be cloned.
    #[must_use]
    pub const fn notifier(&self) -> &LifecycleNotifier {
        &self.notifier
    }
}

impl Stream for LifecycleHandle {
    type Item = LifecycleMessage;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.poll_next_unpin(cx)
    }
}

impl Default for LifecycleHandle {
    fn default() -> Self {
        Self::new()
    }
}
