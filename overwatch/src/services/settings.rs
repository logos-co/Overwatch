use tokio::sync::watch::{channel, Receiver, Sender};
use tracing::error;
#[cfg(feature = "instrumentation")]
use tracing::instrument;

/// Wrapper around [`Receiver`].
pub struct SettingsNotifier<Settings> {
    notifier_channel: Receiver<Settings>,
}

impl<Settings> SettingsNotifier<Settings>
where
    Settings: Clone,
{
    #[must_use]
    pub const fn new(notifier_channel: Receiver<Settings>) -> Self {
        Self { notifier_channel }
    }

    /// Get latest settings.
    ///
    /// It is guaranteed that at least an initial value is present.
    ///
    /// This returns a cloned version of the referenced settings. It simplifies
    /// the API at the expense of some efficiency.
    // TODO: Alternatives:
    // - We can consider returning the Ref<> from the borrowing. This would block the updating
    // channel so this responsibility would be dumped into the end user of the method.
    // - Spawn a task that updates a settings local value each time an updated settings is received.
    // This might be harder to do than it seems since it will need to hold a &mut to the holder
    // (or needed to use a Cell/RefCell).
    #[must_use]
    pub fn get_updated_settings(&self) -> Settings {
        self.notifier_channel.borrow().clone()
    }
}

/// Settings update notification sender.
#[derive(Clone)]
pub struct SettingsUpdater<Settings> {
    sender: Sender<Settings>,
    receiver: Receiver<Settings>,
}

impl<Settings> SettingsUpdater<Settings> {
    pub fn new(settings: Settings) -> Self {
        let (sender, receiver) = channel(settings);

        Self { sender, receiver }
    }

    /// Send a new settings update notification to the watcher end.
    #[cfg_attr(feature = "instrumentation", instrument(skip_all))]
    pub fn update(&self, settings: Settings) {
        self.sender.send(settings).unwrap_or_else(|_e| {
            error!("Error sending settings update for service");
        });
    }

    /// Get a new notifier channel, used to get latest settings changes updates.
    #[must_use]
    pub fn notifier(&self) -> SettingsNotifier<Settings> {
        SettingsNotifier {
            notifier_channel: self.receiver.clone(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, time::Duration};

    use tokio::time::{sleep, timeout};

    use crate::services::settings::SettingsUpdater;

    #[tokio::test]
    async fn settings_updater_sequence() {
        let updater = SettingsUpdater::new(10usize);
        let notifier = updater.notifier();
        let values = [10, 0usize];
        let mut seq = HashSet::from(values);
        let handle = tokio::spawn(timeout(Duration::from_secs(3), async move {
            while !seq.is_empty() {
                let new_value = notifier.get_updated_settings();
                seq.remove(&new_value);
                sleep(Duration::from_millis(50)).await;
            }
            true
        }));
        sleep(Duration::from_millis(100)).await;
        for v in &values[1..] {
            updater.update(*v);
            sleep(Duration::from_millis(100)).await;
        }
        // all values updates have been seen
        let success: Result<bool, _> = handle.await.unwrap();
        assert!(success.unwrap());
    }
}
