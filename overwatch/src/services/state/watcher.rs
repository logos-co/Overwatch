use crate::services::state::Receiver;

/// Receiver part of the state handling mechanism.
pub struct StateWatcher<State> {
    pub(crate) receiver: Receiver<State>,
}

// Clone is implemented manually because auto deriving introduces an unnecessary
// Clone bound on T.
impl<State> Clone for StateWatcher<State> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}

impl<State> StateWatcher<State> {
    /// Create a new [`StateWatcher`]
    #[must_use]
    pub const fn new(receiver: Receiver<State>) -> Self {
        Self { receiver }
    }

    /// Get the internal [`Receiver`].
    #[must_use]
    pub const fn receiver(&self) -> &Receiver<State> {
        &self.receiver
    }
}
