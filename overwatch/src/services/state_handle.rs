use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        life_cycle::LifecycleHandle, relay::InboundRelay, settings::SettingsNotifier,
        state::StateUpdater, status::StatusHandle,
    },
};

/// Core resources for a `Service`.
///
/// Contains everything required to start a new
/// [`ServiceRunner`](crate::services::runner::ServiceRunner).
pub struct ServiceStateHandle<Message, Settings, State, RuntimeServiceId> {
    /// Message channel relay to receive messages from other services
    pub inbound_relay: InboundRelay<Message>,
    pub status_handle: StatusHandle,
    pub overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    pub settings_reader: SettingsNotifier<Settings>,
    pub state_updater: StateUpdater<State>,
    pub lifecycle_handle: LifecycleHandle,
}
