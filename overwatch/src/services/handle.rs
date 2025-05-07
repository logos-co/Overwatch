use tokio::runtime::Handle;

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        life_cycle::LifecycleHandle,
        relay::OutboundRelay,
        settings::SettingsUpdater,
        state::StateHandle,
        status::{StatusHandle, StatusWatcher},
    },
};

/// Handle to a service.
///
/// This is used to access the different components of the `Service`.
// TODO: Abstract handle over state to differentiate when the service is running
// TODO: If this entity is only used by Overwatch, we could get rid of `ServiceRunnerHandle`
//  by storing the Runner's `JoinHandle` here. Currently it's split in two entities to not give
//  access to the `JoinHandle` to unwanted entities.
// and when it is not. That way we could expose a better API depending on what
// is happening and it would get rid of the probably unnecessary Option and
// cloning.
#[derive(Clone)]
pub struct ServiceHandle<Message, Settings, State, Operator, RuntimeServiceId> {
    /// Message channel relay
    ///
    /// It contains the channel if the service is running, otherwise it'll be
    /// [`None`]
    outbound_relay: OutboundRelay<Message>,
    overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    settings_updater: SettingsUpdater<Settings>,
    status_handle: StatusHandle,
    state_handle: StateHandle<State, Operator>,
    lifecycle_handle: LifecycleHandle,
}

impl<Message, Settings, State, Operator, RuntimeServiceId>
    ServiceHandle<Message, Settings, State, Operator, RuntimeServiceId>
{
    /// Crate a new service handle.
    pub const fn new(
        outbound_relay: OutboundRelay<Message>,
        overwatch_handle: OverwatchHandle<RuntimeServiceId>,
        settings_updater: SettingsUpdater<Settings>,
        status_handle: StatusHandle,
        state_handle: StateHandle<State, Operator>,
        lifecycle_handle: LifecycleHandle,
    ) -> Self {
        Self {
            outbound_relay,
            overwatch_handle,
            settings_updater,
            status_handle,
            state_handle,
            lifecycle_handle,
        }
    }

    /// Get the service's [`Handle`].
    ///
    /// It's easily cloneable and can be done on demand.
    pub const fn runtime(&self) -> &Handle {
        self.overwatch_handle.runtime()
    }

    /// Get the service's [`OverwatchHandle`].
    ///
    /// It's easily cloneable and can be done on demand.
    pub const fn overwatch_handle(&self) -> &OverwatchHandle<RuntimeServiceId> {
        &self.overwatch_handle
    }

    /// Request a relay to this service.
    ///
    /// If the service is not running, it will return [`None`].
    pub fn relay_with(&self) -> OutboundRelay<Message> {
        self.outbound_relay.clone()
    }

    /// Get the [`StatusWatcher`] for this service.
    pub fn status_watcher(&self) -> StatusWatcher {
        self.status_handle.watcher()
    }

    /// Update the current settings with a new one.
    pub fn update_settings(&self, settings: Settings) {
        self.settings_updater.update(settings);
    }

    /// Get the [`StatusHandle`] for this service.
    pub const fn status_handle(&self) -> &StatusHandle {
        &self.status_handle
    }

    /// Get the [`StateHandle`] for this service.
    pub const fn state_handle(&self) -> &StateHandle<State, Operator> {
        &self.state_handle
    }

    /// Get the [`LifecycleHandle`] for this service.
    pub const fn lifecycle_handle(&self) -> &LifecycleHandle {
        &self.lifecycle_handle
    }
}
