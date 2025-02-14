// crates
use tokio::runtime::Handle;
use tracing::info;
// internal
use crate::overwatch::handle::OverwatchHandle;
use crate::services::life_cycle::LifecycleHandle;
use crate::services::relay::{relay, InboundRelay, OutboundRelay};
use crate::services::settings::{SettingsNotifier, SettingsUpdater};
use crate::services::state::{StateHandle, StateOperator, StateUpdater};
use crate::services::status::{StatusHandle, StatusWatcher};
use crate::services::{ServiceCore, ServiceId, ServiceState};

// TODO: Abstract handle over state, to differentiate when the service is running and when it is not
// that way we can expose a better API depending on what is happenning. Would get rid of the probably
// unnecessary Option and cloning.
/// Service handle
/// This is used to access different parts of the service
pub struct ServiceHandle<Message, Settings, State> {
    /// Message channel relay
    /// Would be None if service is not running
    /// Will contain the channel if service is running
    outbound_relay: Option<OutboundRelay<Message>>,
    /// Handle to overwatch
    overwatch_handle: OverwatchHandle,
    settings: SettingsUpdater<Settings>,
    status: StatusHandle,
    initial_state: State,
    relay_buffer_size: usize,
}

impl<Message, Settings, State> ServiceHandle<Message, Settings, State>
where
    Settings: Clone,
    State: ServiceState<Settings = Settings> + Clone,
{
    pub fn new<StateOp>(
        settings: Settings,
        overwatch_handle: OverwatchHandle,
        relay_buffer_size: usize,
    ) -> Result<Self, State::Error>
    where
        StateOp: StateOperator<Settings = Settings, StateInput = State>,
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

    /// Service runtime getter
    /// it is easily cloneable and can be done on demand
    pub fn runtime(&self) -> &Handle {
        self.overwatch_handle.runtime()
    }

    /// Overwatch handle
    /// it is easily cloneable and can be done on demand
    pub fn overwatch_handle(&self) -> &OverwatchHandle {
        &self.overwatch_handle
    }

    /// Request a relay with this service
    pub fn relay_with(&self) -> Option<OutboundRelay<Message>> {
        self.outbound_relay.clone()
    }

    pub fn status_watcher(&self) -> StatusWatcher {
        self.status.watcher()
    }

    /// Update settings
    pub fn update_settings(&self, settings: Settings) {
        self.settings.update(settings);
    }

    /// Build a runner for this service
    pub fn service_runner<StateOp>(&mut self) -> ServiceRunner<Message, Settings, State, StateOp>
    where
        StateOp: StateOperator<Settings = Settings>,
    {
        // TODO: add proper status handling here, a service should be able to produce a runner if it is already running.
        let (inbound_relay, outbound_relay) = relay::<Message>(self.relay_buffer_size);
        let settings_reader = self.settings.notifier();
        // add relay channel to handle
        self.outbound_relay = Some(outbound_relay);
        let settings = self.settings.notifier().get_updated_settings();
        let operator = StateOp::from_settings(settings);
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

        ServiceRunner {
            service_state,
            state_handle,
            lifecycle_handle,
            initial_state: self.initial_state.clone(),
        }
    }
}

/// Service core resources
/// It contains whatever is necessary to start a new service runner
pub struct ServiceStateHandle<Message, Settings, State> {
    /// Relay channel to communicate with the service runner
    pub inbound_relay: InboundRelay<Message>,
    pub status_handle: StatusHandle,
    /// Overwatch handle
    pub overwatch_handle: OverwatchHandle,
    pub settings_reader: SettingsNotifier<Settings>,
    pub state_updater: StateUpdater<State>,
    pub lifecycle_handle: LifecycleHandle,
}

/// Main service executor
/// It is the object that hold the necessary information for the service to run
pub struct ServiceRunner<Message, Settings, State, StateOperator> {
    service_state: ServiceStateHandle<Message, Settings, State>,
    state_handle: StateHandle<State, StateOperator>,
    lifecycle_handle: LifecycleHandle,
    initial_state: State,
}

impl<Message, Settings, State, StateOp> ServiceRunner<Message, Settings, State, StateOp>
where
    State: Clone + Send + Sync + 'static,
    StateOp: StateOperator<StateInput = State> + Send + 'static,
{
    /// Spawn the service main loop and handle it lifecycle
    /// Return a handle to abort execution manually
    pub fn run<Service>(self) -> Result<(ServiceId, LifecycleHandle), crate::DynError>
    where
        Service: ServiceCore<Settings = Settings, State = State, Message = Message> + 'static,
    {
        let ServiceRunner {
            service_state,
            state_handle,
            lifecycle_handle,
            initial_state,
            ..
        } = self;

        let runtime = service_state.overwatch_handle.runtime().clone();
        let service = Service::init(service_state, initial_state)?;

        runtime.spawn(service.run());
        runtime.spawn(state_handle.run());

        Ok((Service::SERVICE_ID, lifecycle_handle))
    }
}
