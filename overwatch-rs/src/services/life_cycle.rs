// std
use futures::Stream;
use std::default::Default;
use std::error::Error;
// crates
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio_stream::StreamExt;
// internal
use crate::DynError;

/// Type alias for an empty signal
pub type FinishedSignal = ();

/// Supported lifecycle messages
#[derive(Clone, Debug)]
pub enum LifecycleMessage {
    /// Hold a sender from a broadcast channel. This is used to signal when the service has finished
    /// handling the shutdown process.
    Shutdown(Sender<FinishedSignal>),
    Kill,
}

/// Handle for lifecycle communications with a `Service`
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

impl LifecycleHandle {
    #[must_use]
    pub fn new() -> Self {
        // Process a single [`LifecycleMessage`] at a time.
        // All lifecycle computations are processed sequentially to prevent race conditions
        // (e.g.: unordered messages).
        // [`LifecycleMessage`] senders wait until the channel is empty before sending a new
        // message, akin to a mutex.
        let (notifier, message_channel) = channel(1);
        Self {
            message_channel,
            notifier,
        }
    }

    /// Incoming [`LifecycleMessage`] stream
    /// Note that messages are not buffered: Different calls to this method could yield different
    /// messages depending on when the method is called.
    pub fn message_stream(&self) -> impl Stream<Item = LifecycleMessage> {
        tokio_stream::wrappers::BroadcastStream::new(self.message_channel.resubscribe())
            .filter_map(Result::ok)
    }

    /// Send a [`LifecycleMessage`] to the service
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
