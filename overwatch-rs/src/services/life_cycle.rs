use crate::DynError;
use futures::Stream;
use std::default::Default;
use std::error::Error;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio_stream::StreamExt;

pub type FinishedSignal = ();

#[derive(Clone, Debug)]
pub enum LifecycleMessage {
    Shutdown(Sender<FinishedSignal>),
    Kill,
}

pub struct LifecycleHandle {
    message_channel: Receiver<LifecycleMessage>,
    notifier: Sender<LifecycleMessage>,
}

impl Clone for LifecycleHandle {
    fn clone(&self) -> Self {
        Self {
            message_channel: self.message_channel.resubscribe(),
            notifier: self.notifier.clone(),
        }
    }
}

impl LifecycleHandle {
    pub fn new() -> Self {
        // Use a single lifecycle message at a time. Idea is that all computations on lifecycle should
        // stack so waiting es effective even if later on is somehow reversed (for example for start/stop events).
        let (notifier, message_channel) = channel(1);
        Self {
            notifier,
            message_channel,
        }
    }
    pub fn message_stream(&self) -> impl Stream<Item = LifecycleMessage> {
        tokio_stream::wrappers::BroadcastStream::new(self.message_channel.resubscribe())
            .filter_map(Result::ok)
    }

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
