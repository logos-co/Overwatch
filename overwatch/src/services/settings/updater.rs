use tokio::sync::watch::Sender;
use tracing::error;
#[cfg(feature = "instrumentation")]
use tracing::instrument;

#[derive(Clone)]
pub struct SettingsUpdater<Settings> {
    sender: Sender<Settings>,
}

impl<Settings> SettingsUpdater<Settings> {
    #[must_use]
    pub const fn new(sender: Sender<Settings>) -> Self {
        Self { sender }
    }

    /// Send a new settings update notification to the watcher end.
    #[cfg_attr(feature = "instrumentation", instrument(skip_all))]
    pub fn update(&self, settings: Settings) {
        self.sender.send(settings).unwrap_or_else(|error| {
            error!("Error sending settings update for service: {error}");
        });
    }
}
