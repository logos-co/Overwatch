use tokio::runtime::Handle;
use tracing::info;

use crate::services::state::StateUpdater;
use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        life_cycle::LifecycleHandle,
        relay::{OutboundRelay, Relay},
        runner::ServiceRunner,
        settings::SettingsUpdater,
        state::{StateHandle, StateOperator},
        state_handle::ServiceResources,
        status::{StatusHandle, StatusWatcher},
        ServiceState,
    },
};

/// Handle to a service.
///
/// This is used to access the different components of the `Service`.
// TODO: Abstract handle over state to differentiate when the service is running
// and when it is not. That way we could expose a better API depending on what
// is happening and it would get rid of the probably unnecessary Option and
// cloning.
pub struct ServiceHandle<Message, Settings, State, Operator, RuntimeServiceId> {
    /// Message channel relay
    ///
    /// It contains the channel if the service is running, otherwise it'll be
    /// [`None`]
    outbound_relay: Option<OutboundRelay<Message>>,
    overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    settings_updater: SettingsUpdater<Settings>,
    status: StatusHandle,
    state_handle: StateHandle<State, Operator>,
    state_updater: StateUpdater<State>, // TODO: Remove this. It's not needed.
    relay_buffer_size: usize,
}

impl<Message, Settings, State, Operator, RuntimeServiceId>
    ServiceHandle<Message, Settings, State, Operator, RuntimeServiceId>
where
    Settings: Clone,
    State: ServiceState<Settings = Settings> + Clone,
    RuntimeServiceId: Clone,
    Operator: StateOperator<State = State> + Clone,
{
    /// Crate a new service handle.
    ///
    /// # Errors
    ///
    /// If the service state cannot be loaded from the provided settings.
    pub fn new(
        settings: Settings,
        overwatch_handle: OverwatchHandle<RuntimeServiceId>,
        lifecycle_handle: LifecycleHandle,
        relay_buffer_size: usize,
    ) -> Result<Self, State::Error> {
        let settings_updater = SettingsUpdater::new(settings);
        let settings = settings_updater.notifier().get_updated_settings();
        let operator = Operator::from_settings(&settings);

        // TODO: Remove. Initial state should be in runner, as well as state stuff.
        let initial_state = if let Ok(Some(loaded_state)) = Operator::try_load(&settings) {
            info!("Loaded state from Operator");
            loaded_state
        } else {
            info!("Couldn't load state from Operator. Creating from settings.");
            State::from_settings(&settings)?
        };
        let (state_handle, state_updater) =
            StateHandle::<State, Operator>::new(initial_state, operator);

        Ok(Self {
            outbound_relay: None,
            overwatch_handle,
            settings_updater,
            status: StatusHandle::new(),
            state_handle,
            state_updater,
            relay_buffer_size,
        })
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
    pub fn relay_with(&self) -> Option<OutboundRelay<Message>> {
        self.outbound_relay.clone()
    }

    /// Get the [`StatusWatcher`] for this service.
    pub fn status_watcher(&self) -> StatusWatcher {
        self.status.watcher()
    }

    /// Update the current settings with a new one.
    pub fn update_settings(&self, settings: Settings) {
        self.settings_updater.update(settings);
    }

    /// Build a runner for this service
    pub fn service_runner(
        &mut self,
    ) -> ServiceRunner<Message, Settings, State, Operator, RuntimeServiceId> {
        // TODO: Add proper status handling here.
        // A service should be able to produce a runner if it is already running.

        let Relay {
            inbound,
            outbound,
            consumer_sender,
            consumer_receiver,
        } = Relay::new(self.relay_buffer_size);

        let settings_reader = self.settings_updater.notifier();
        // Add relay channel to handle
        self.outbound_relay = Some(outbound);

        let lifecycle_handle = LifecycleHandle::new();

        let service_resources = ServiceResources::new(
            self.status.clone(),
            self.overwatch_handle.clone(),
            settings_reader,
            self.state_updater.clone(),
            lifecycle_handle.clone(),
        );

        ServiceRunner::new(
            service_resources,
            self.state_handle.clone(),
            lifecycle_handle,
            inbound,
            consumer_sender,
            consumer_receiver,
            self.relay_buffer_size,
        )
    }
}
