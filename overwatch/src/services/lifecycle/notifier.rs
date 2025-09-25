use tokio::sync::mpsc::Sender;

use crate::{DynError, services::lifecycle::message::LifecycleMessage};

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
