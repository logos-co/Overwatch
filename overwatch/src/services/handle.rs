use tokio::runtime::Handle;
use tracing::info;

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        life_cycle::LifecycleHandle,
        relay::{relay, OutboundRelay},
        runner::ServiceRunner,
        settings::SettingsUpdater,
        state::{StateHandle, StateOperator},
        state_handle::ServiceStateHandle,
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
pub struct ServiceHandle<Message, Settings, State, RuntimeServiceId> {
    /// Message channel relay
    ///
    /// It contains the channel if the service is running, otherwise it'll be
    /// [`None`]
    outbound_relay: Option<OutboundRelay<Message>>,
    overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    settings: SettingsUpdater<Settings>,
    status: StatusHandle,
    initial_state: State,
    relay_buffer_size: usize,
}

impl<Message, Settings, State, RuntimeServiceId>
    ServiceHandle<Message, Settings, State, RuntimeServiceId>
where
    Settings: Clone,
    State: ServiceState<Settings = Settings> + Clone,
    RuntimeServiceId: Clone,
{
    /// Crate a new service handle.
    ///
    /// # Errors
    ///
    /// If the service state cannot be loaded from the provided settings.
    pub fn new<StateOp>(
        settings: Settings,
        overwatch_handle: OverwatchHandle<RuntimeServiceId>,
        relay_buffer_size: usize,
    ) -> Result<Self, State::Error>
    where
        StateOp: StateOperator<State = State>,
    {
        let initial_state = if let Ok(Some(loaded_state)) = StateOp::try_load(&settings) {
            info!("Loaded state from Operator");
            loaded_state
        } else {
            info!("Couldn't load state from Operator. Creating from settings.");
            State::from_settings(&settings)?
        };

        Ok(Self {
            outbound_relay: None,
            overwatch_handle,
            settings: SettingsUpdater::new(settings),
            status: StatusHandle::new(),
            initial_state,
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
        self.settings.update(settings);
    }

    /// Build a runner for this service
    pub fn service_runner<StateOp>(
        &mut self,
    ) -> ServiceRunner<Message, Settings, State, StateOp, RuntimeServiceId>
    where
        StateOp: StateOperator<State = State>,
    {
        // TODO: Add proper status handling here.
        // A service should be able to produce a runner if it is already running.
        let (inbound_relay, outbound_relay) = relay::<Message>(self.relay_buffer_size);
        let settings_reader = self.settings.notifier();
        // Add relay channel to handle
        self.outbound_relay = Some(outbound_relay);
        let settings = self.settings.notifier().get_updated_settings();
        let operator = StateOp::from_settings(&settings);
        let (state_handle, state_updater) =
            StateHandle::<State, StateOp>::new(self.initial_state.clone(), operator);

        let lifecycle_handle = LifecycleHandle::new();

        let service_state = ServiceStateHandle {
            inbound_relay,
            status_handle: self.status.clone(),
            overwatch_handle: self.overwatch_handle.clone(),
            state_updater,
            settings_reader,
            lifecycle_handle: lifecycle_handle.clone(),
        };

        ServiceRunner::new(
            service_state,
            state_handle,
            lifecycle_handle,
            self.initial_state.clone(),
        )
    }
}
