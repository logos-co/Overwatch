use tokio::sync::watch::channel;

use crate::services::settings::{notifier::SettingsNotifier, updater::SettingsUpdater};

/// Settings update notification sender.
#[derive(Clone)]
pub struct SettingsHandle<Settings> {
    updater: SettingsUpdater<Settings>,
    notifier: SettingsNotifier<Settings>,
}

impl<Settings> SettingsHandle<Settings>
where
    Settings: Clone,
{
    pub fn new(settings: Settings) -> Self {
        let (sender, receiver) = channel(settings);
        let updater = SettingsUpdater::new(sender);
        let notifier = SettingsNotifier::new(receiver);
        Self { updater, notifier }
    }

    #[must_use]
    pub const fn updater(&self) -> &SettingsUpdater<Settings> {
        &self.updater
    }

    #[must_use]
    pub const fn notifier(&self) -> &SettingsNotifier<Settings> {
        &self.notifier
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, time::Duration};

    use tokio::time::{sleep, timeout};

    use super::*;

    #[tokio::test]
    async fn settings_updater_sequence() {
        let SettingsHandle { notifier, updater } = SettingsHandle::new(10usize);
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
