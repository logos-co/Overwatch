use std::marker::PhantomData;

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        life_cycle::LifecycleHandle, relay::InboundRelay, settings::SettingsNotifier,
        state::StateUpdater, status::StatusHandle,
    },
};
// TODO: RENAME FILE

/// Core resources for a `Service`.
///
/// Contains everything required to start a new
/// [`ServiceRunner`](crate::services::runner::ServiceRunner).
pub struct ServiceResources<Message, Settings, State, RuntimeServiceId> {
    /// Message channel relay to receive messages from other services
    pub status_handle: StatusHandle,
    pub overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    pub settings_reader: SettingsNotifier<Settings>, /* TODO: Use SettingsUpdater and call
                                                      * .notifier */
    pub state_updater: StateUpdater<State>,
    pub lifecycle_handle: LifecycleHandle,
    _message: PhantomData<Message>,
}

impl<Message, Settings, State, RuntimeServiceId>
    ServiceResources<Message, Settings, State, RuntimeServiceId>
where
    RuntimeServiceId: Clone,
    Settings: Clone,
{
    #[must_use]
    pub const fn new(
        status_handle: StatusHandle,
        overwatch_handle: OverwatchHandle<RuntimeServiceId>,
        settings_reader: SettingsNotifier<Settings>,
        state_updater: StateUpdater<State>,
        lifecycle_handle: LifecycleHandle,
    ) -> Self {
        Self {
            status_handle,
            overwatch_handle,
            settings_reader,
            state_updater,
            lifecycle_handle,
            _message: PhantomData,
        }
    }

    /// Create a new [`ServiceResourcesHandle`](ServiceResourcesHandle) from the
    /// current `ServiceResources`.
    /// TODO: Needed extra inbound_relay?
    #[must_use]
    pub fn to_handle(
        &self,
        inbound_relay: InboundRelay<Message>,
    ) -> ServiceResourcesHandle<Message, Settings, State, RuntimeServiceId> {
        ServiceResourcesHandle {
            inbound_relay,
            status_handle: self.status_handle.clone(),
            overwatch_handle: self.overwatch_handle.clone(),
            settings_reader: self.settings_reader.clone(),
            state_updater: self.state_updater.clone(),
            lifecycle_handle: self.lifecycle_handle.clone(),
        }
    }
}

pub struct ServiceResourcesHandle<Message, Settings, State, RuntimeServiceId> {
    pub inbound_relay: InboundRelay<Message>,
    pub status_handle: StatusHandle,
    pub overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    pub settings_reader: SettingsNotifier<Settings>,
    pub state_updater: StateUpdater<State>,
    pub lifecycle_handle: LifecycleHandle,
}
