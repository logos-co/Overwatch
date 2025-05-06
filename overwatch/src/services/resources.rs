use std::marker::PhantomData;

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        life_cycle::LifecycleHandle, relay::InboundRelay, settings::SettingsUpdater,
        state::StateUpdater, status::StatusHandle,
    },
};

/// Core resources for a `Service`.
///
/// Contains everything required to start a new
/// [`ServiceRunner`](crate::services::runner::ServiceRunner).
pub struct ServiceResources<Message, Settings, State, RuntimeServiceId> {
    /// Message channel relay to receive messages from other services
    pub status_handle: StatusHandle,
    pub overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    pub settings_updater: SettingsUpdater<Settings>,
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
        settings_updater: SettingsUpdater<Settings>,
        state_updater: StateUpdater<State>,
        lifecycle_handle: LifecycleHandle,
    ) -> Self {
        Self {
            status_handle,
            overwatch_handle,
            settings_updater,
            state_updater,
            lifecycle_handle,
            _message: PhantomData,
        }
    }

    /// Create a new [`ServiceResourcesHandle`](ServiceResourcesHandle) from the
    /// current `ServiceResources`.
    ///
    /// # Parameters
    ///
    /// * `inbound_relay`: The relay the service will use to receive messages.
    ///   Due to the singleton nature of the inbound relay, if the recipient
    ///   service is being restarted, then the relay should be same one returned
    ///   by the previous instance when it was stopped. This ensures the new
    ///   instance will maintain communication with other services who opened a
    ///   relay to the previous instance.
    #[must_use]
    pub fn to_handle(
        &self,
        inbound_relay: InboundRelay<Message>,
    ) -> ServiceResourcesHandle<Message, Settings, State, RuntimeServiceId> {
        ServiceResourcesHandle {
            inbound_relay,
            status_handle: self.status_handle.clone(),
            overwatch_handle: self.overwatch_handle.clone(),
            settings_updater: self.settings_updater.clone(),
            state_updater: self.state_updater.clone(),
            lifecycle_handle: self.lifecycle_handle.clone(),
        }
    }
}

pub struct ServiceResourcesHandle<Message, Settings, State, RuntimeServiceId> {
    pub inbound_relay: InboundRelay<Message>,
    pub status_handle: StatusHandle,
    pub overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    pub settings_updater: SettingsUpdater<Settings>,
    pub state_updater: StateUpdater<State>,
    pub lifecycle_handle: LifecycleHandle, // TODO: Removing the ability of services to interact
                                           //  with their lifecycle is probably a good idea.
                                           //  Fetching data this way can lead to deadlocks.
}
