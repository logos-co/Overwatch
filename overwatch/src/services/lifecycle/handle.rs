use std::{
    default::Default,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{Stream, StreamExt};
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;

use crate::services::lifecycle::{LifecycleMessage, LifecycleNotifier};

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
