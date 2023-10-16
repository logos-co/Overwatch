// crates
use tokio::runtime::Handle;
// internal
use crate::overwatch::handle::OverwatchHandle;
use crate::services::life_cycle::LifecycleHandle;
use crate::services::relay::{relay_state, InboundRelayState, OutboundRelayState};
use crate::services::settings::{SettingsNotifier, SettingsUpdater};
use crate::services::state::{StateHandle, StateOperator, StateUpdater};
use crate::services::{ServiceCore, ServiceData, ServiceId, ServiceState};

// TODO: Abstract handle over state, to differentiate when the service is running and when it is not
// that way we can expose a better API depending on what is happenning. Would get rid of the probably
// unnecessary Option and cloning.
/// Service handle
/// This is used to access different parts of the service
pub struct ServiceHandle<S: ServiceData> {
    /// Message channel relay
    outbound_relay: OutboundRelayState<S::Message>,
    inbound_tmp: Option<InboundRelayState<S::Message>>,
    /// Handle to overwatch
    overwatch_handle: OverwatchHandle,
    settings: SettingsUpdater<S::Settings>,
    initial_state: S::State,
}

/// Service core resources
/// It contains whatever is necessary to start a new service runner
pub struct ServiceStateHandle<S: ServiceData> {
    /// Relay channel to communicate with the service runner
    pub inbound_relay: InboundRelayState<S::Message>,
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
}

impl<S: ServiceData> ServiceHandle<S> {
    pub fn new(
        settings: S::Settings,
        overwatch_handle: OverwatchHandle,
    ) -> Result<Self, <S::State as ServiceState>::Error> {
        let (inbound_tmp, outbound_relay) = relay_state::<S::Message>();
        S::State::from_settings(&settings).map(|initial_state| Self {
            outbound_relay,
            inbound_tmp: Some(inbound_tmp),
            overwatch_handle,
            settings: SettingsUpdater::new(settings),
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
    pub fn relay_with(&self) -> OutboundRelayState<S::Message> {
        self.outbound_relay.clone()
    }

    /// Update settings
    pub fn update_settings(&self, settings: S::Settings) {
        self.settings.update(settings)
    }

    /// Build a runner for this service
    pub fn service_runner(&mut self) -> ServiceRunner<S> {
        let settings_reader = self.settings.notifier();
        let settings = self.settings.notifier().get_updated_settings();
        let operator = S::StateOperator::from_settings::<S::Settings>(settings);
        let (state_handle, state_updater) =
            StateHandle::<S::State, S::StateOperator>::new(self.initial_state.clone(), operator);

        let lifecycle_handle = LifecycleHandle::new();
        let inbound_relay = self
            .inbound_tmp
            .take()
            .expect("Inbound channel mut be present at initialization");
        let service_state = ServiceStateHandle {
            inbound_relay,
            overwatch_handle: self.overwatch_handle.clone(),
            state_updater,
            settings_reader,
            lifecycle_handle: lifecycle_handle.clone(),
        };

        ServiceRunner {
            service_state,
            state_handle,
            lifecycle_handle,
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
            mut service_state,
            state_handle,
            lifecycle_handle,
        } = self;

        let runtime = service_state.overwatch_handle.runtime().clone();
        service_state.inbound_relay = service_state
            .inbound_relay
            .connect(S::SERVICE_RELAY_BUFFER_SIZE);
        let service = S::init(service_state)?;

        runtime.spawn(service.run());
        runtime.spawn(state_handle.run());

        Ok((S::SERVICE_ID, lifecycle_handle))
    }
}
