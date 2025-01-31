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
use crate::services::{ServiceCore, ServiceData, ServiceId, ServiceState};

// TODO: Abstract handle over state, to differentiate when the service is running and when it is not
// that way we can expose a better API depending on what is happenning. Would get rid of the probably
// unnecessary Option and cloning.
/// Service handle
/// This is used to access different parts of the service
pub struct ServiceHandle<S: ServiceData> {
    /// Message channel relay
    /// Would be None if service is not running
    /// Will contain the channel if service is running
    outbound_relay: Option<OutboundRelay<S::Message>>,
    /// Handle to overwatch
    overwatch_handle: OverwatchHandle,
    settings: SettingsUpdater<S::Settings>,
    status: StatusHandle<S>,
    initial_state: S::State,
}

/// Service core resources
/// It contains whatever is necessary to start a new service runner
pub struct ServiceStateHandle<S: ServiceData> {
    /// Relay channel to communicate with the service runner
    pub inbound_relay: InboundRelay<S::Message>,
    pub status_handle: StatusHandle<S>,
    /// Overwatch handle
    pub overwatch_handle: OverwatchHandle,
    pub settings_reader: SettingsNotifier<S::Settings>,
    pub state_updater: StateUpdater<S::State>,
    pub lifecycle_handle: LifecycleHandle,
}

/// Main service executor
/// It is the object that hold the necessary information for the service to run
pub struct ServiceRunner<S: ServiceData> {
    service_state: ServiceStateHandle<S>,
    state_handle: StateHandle<S::State, S::StateOperator>,
    lifecycle_handle: LifecycleHandle,
    initial_state: S::State,
}

impl<S: ServiceData> ServiceHandle<S> {
    pub fn new(
        settings: S::Settings,
        overwatch_handle: OverwatchHandle,
    ) -> Result<Self, <S::State as ServiceState>::Error> {
        let initial_state = if let Ok(Some(loaded_state)) = S::StateOperator::try_load(&settings) {
            info!("Loaded state from Operator");
            loaded_state
        } else {
            info!("Couldn't load state from Operator. Creating from settings.");
            S::State::from_settings(&settings)?
        };

        Ok(Self {
            outbound_relay: None,
            overwatch_handle,
            settings: SettingsUpdater::new(settings),
            status: StatusHandle::new(),
            initial_state,
        })
    }

    pub fn id(&self) -> ServiceId {
        S::SERVICE_ID
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
    pub fn relay_with(&self) -> Option<OutboundRelay<S::Message>> {
        self.outbound_relay.clone()
    }

    pub fn status_watcher(&self) -> StatusWatcher {
        self.status.watcher()
    }

    /// Update settings
    pub fn update_settings(&self, settings: S::Settings) {
        self.settings.update(settings)
    }

    /// Build a runner for this service
    pub fn service_runner(&mut self) -> ServiceRunner<S> {
        // TODO: add proper status handling here, a service should be able to produce a runner if it is already running.
        let (inbound_relay, outbound_relay) = relay::<S::Message>(S::SERVICE_RELAY_BUFFER_SIZE);
        let settings_reader = self.settings.notifier();
        // add relay channel to handle
        self.outbound_relay = Some(outbound_relay);
        let settings = self.settings.notifier().get_updated_settings();
        let operator = S::StateOperator::from_settings(settings);
        let (state_handle, state_updater) =
            StateHandle::<S::State, S::StateOperator>::new(self.initial_state.clone(), operator);

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

impl<S: ServiceData> ServiceStateHandle<S> {
    pub fn id(&self) -> ServiceId {
        S::SERVICE_ID
    }
}

impl<S> ServiceRunner<S>
where
    S::State: Send + Sync + 'static,
    S::StateOperator: Send + 'static,
    S: ServiceCore + 'static,
{
    /// Spawn the service main loop and handle it lifecycle
    /// Return a handle to abort execution manually
    pub fn run(self) -> Result<(ServiceId, LifecycleHandle), crate::DynError> {
        let ServiceRunner {
            service_state,
            state_handle,
            lifecycle_handle,
            initial_state,
        } = self;

        let runtime = service_state.overwatch_handle.runtime().clone();
        let service = S::init(service_state, initial_state)?;

        runtime.spawn(service.run());
        runtime.spawn(state_handle.run());

        Ok((S::SERVICE_ID, lifecycle_handle))
    }
}
