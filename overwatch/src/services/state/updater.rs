use std::sync::Arc;

use tracing::error;

use crate::services::state::Sender;

/// Sender part of the state handling mechanism.
///
/// Update the current state and notifies the [`StateHandle`].
pub struct StateUpdater<State> {
    sender: Arc<Sender<State>>,
}

// Clone is implemented manually because auto deriving introduces an unnecessary
//  Clone bound on T.
impl<State> Clone for StateUpdater<State> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<State> StateUpdater<State> {
    /// Create a new [`StateUpdater`]
    #[must_use]
    pub const fn new(sender: Arc<Sender<State>>) -> Self {
        Self { sender }
    }

    /// Send a new state and notify the [`StateWatcher`].
    ///
    /// `None` values won't be forwarded to the [`StateOperator`].
    pub fn update(&self, new_state: State) {
        self.sender.send(new_state).unwrap_or_else(|error| {
            error!("Error updating State: {error}");
        });
    }
}
