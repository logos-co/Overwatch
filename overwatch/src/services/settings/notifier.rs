use tokio::sync::watch::Receiver;

/// Wrapper around [`Receiver`].
#[derive(Clone)]
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

    /// Get the latest settings.
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
