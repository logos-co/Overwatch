use std::{default::Default, error::Error};

use futures::Stream;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio_stream::StreamExt;

use crate::DynError;

/// Type alias for an empty signal.
pub type FinishedSignal = ();

/// Message type for `Service` lifecycle events.
#[derive(Clone, Debug)]
pub enum LifecycleMessage {
    /// Starts the `Service`.
    ///
    /// If the `Service` has been stopped with [`LifecycleMessage::Stop`], it
    /// can be restarted.
    ///
    /// # Arguments
    ///
    /// - [`Sender<FinishedSignal>`]: A [`FinishedSignal`] will be sent through
    ///   the associated channel upon completion of the task.
    Start(Sender<FinishedSignal>),

    /// Stops the `Service`.
    ///
    /// Inner `Service` operations are not guaranteed to be completed.
    /// Despite that, `Service`s stopped this way can be restarted (from a
    /// previously saved point or from the default initial state) by sending
    /// a [`LifecycleMessage::Start`].
    ///
    /// # Arguments
    ///
    /// - [`Sender<FinishedSignal>`]: A [`FinishedSignal`] will be sent through
    ///   the associated channel upon completion of the task.
    Stop(Sender<FinishedSignal>),
}

/// Handle for lifecycle communications with a `Service`.
pub struct LifecycleHandle {
    message_channel: Receiver<LifecycleMessage>,
    notifier: Sender<LifecycleMessage>,
}

impl Clone for LifecycleHandle {
    fn clone(&self) -> Self {
        Self {
            // `resubscribe` gives access only to newly produced events, not already enqueued ones.
            // This is acceptable for two reasons:
            // - Signals that were lost were no longer relevant at the time they were produced.
            // - The entity holding the handle was likely no longer active.
            message_channel: self.message_channel.resubscribe(),
            notifier: self.notifier.clone(),
        }
    }
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
        let (notifier, message_channel) = channel(1);
        Self {
            message_channel,
            notifier,
        }
    }

    /// Incoming [`LifecycleMessage`] stream for the `Service`.
    ///
    /// Note that messages are not buffered: Different calls to this method
    /// could yield different messages depending on when the method is
    /// called.
    pub fn message_stream(&self) -> impl Stream<Item = LifecycleMessage> {
        tokio_stream::wrappers::BroadcastStream::new(self.message_channel.resubscribe())
            .filter_map(Result::ok)
    }

    /// Send a [`LifecycleMessage`] to the `Service`.
    ///
    /// # Errors
    ///
    /// If the message cannot be sent to the service.
    pub fn send(&self, msg: LifecycleMessage) -> Result<(), DynError> {
        self.notifier
            .send(msg)
            .map(|_| ())
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync + 'static>)
    }
}

impl Default for LifecycleHandle {
    fn default() -> Self {
        Self::new()
    }
}
