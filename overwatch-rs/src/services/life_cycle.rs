use crate::DynError;
use futures::Stream;
use std::error::Error;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio_stream::StreamExt;

type FinishedSignal = ();

#[derive(Clone, Debug)]
pub enum LifecycleMessage {
    Shutdown(Sender<FinishedSignal>),
    Kill,
}

#[derive(Clone)]
pub struct LifecycleNotifier(Sender<LifecycleMessage>);

pub struct LifecycleHandle {
    message_channel: Receiver<LifecycleMessage>,
}

impl LifecycleNotifier {
    pub fn send(&self, msg: LifecycleMessage) -> Result<(), DynError> {
        self.0
            .send(msg)
            .map(|_| ())
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync + 'static>)
    }
}

impl LifecycleHandle {
    pub fn into_message_stream(self) -> impl Stream<Item = LifecycleMessage> {
        tokio_stream::wrappers::BroadcastStream::new(self.message_channel).filter_map(Result::ok)
    }
}

pub fn new_lifecycle_channel() -> (LifecycleNotifier, LifecycleHandle) {
    // Use a single lifecycle message at a time. Idea is that all computations on lifecycle should
    // stack so waiting es effective even if later on is somehow reversed (for example for start/stop events).
    let (sender, message_channel) = channel(1);
    (
        LifecycleNotifier(sender),
        LifecycleHandle { message_channel },
    )
}
