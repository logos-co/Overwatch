//std
//crates
use tokio::sync::watch::{channel, Receiver, Sender};
use tracing::{error, instrument};
//internal

/// Wrapper around [`tokio::sync::watch::Receiver`]
pub struct SettingsNotifier<S> {
    notifier_channel: Receiver<S>,
}

impl<S: Clone> SettingsNotifier<S> {
    pub fn new(notifier_channel: Receiver<S>) -> Self {
        Self { notifier_channel }
    }

    /// Get latest settings, it is guaranteed that at least an initial value is present
    /// This returns a cloned version of the referenced settings. It simplifies the API
    /// at the expense of some efficiency.
    // TODO: Maybe we can consider returning the Ref<> from the borrowing. But in doing would be
    // be blocking the updating channel so this responsibility would be dumped into the end user
    // of the method. Another option would be to spawn a task that updates a settings local value
    // each time an updated settings is received. This could not be so easy to do, since it will
    // need to hold a &mut to the holder (or needed to use a Cell/RefCell).
    pub fn get_updated_settings(&self) -> S {
        self.notifier_channel.borrow().clone()
    }
}

/// Settings update notification sender
pub struct SettingsUpdater<S> {
    sender: Sender<S>,
    receiver: Receiver<S>,
}

impl<S> SettingsUpdater<S> {
    pub fn new(settings: S) -> Self {
        let (sender, receiver) = channel(settings);

        Self { sender, receiver }
    }

    /// Send a new settings update notification to the watcher end
    #[instrument(skip_all)]
    pub fn update(&self, settings: S) {
        self.sender.send(settings).unwrap_or_else(|_e| {
            error!("Error sending settings update for service");
        });
    }

    /// Get a new notifier channel, used to get latest settings changes updates
    pub fn notifier(&self) -> SettingsNotifier<S> {
        SettingsNotifier {
            notifier_channel: self.receiver.clone(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::services::settings::SettingsUpdater;
    use std::collections::HashSet;
    use std::time::Duration;
    use tokio::time::sleep;
    use tokio::time::timeout;

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
