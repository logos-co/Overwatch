use tracing::info;

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        handle::ServiceHandle,
        life_cycle::LifecycleHandle,
        relay::{ConsumerReceiver, ConsumerSender, InboundRelay, OutboundRelay, Relay},
        settings::{SettingsNotifier, SettingsUpdater},
        state::{
            fuse, ServiceState, StateHandle, StateOperator as StateOperatorTrait, StateUpdater,
        },
        status::{handle::ServiceAPI, StatusHandle, StatusUpdater},
    },
};

/// Core resources for a `Service`.
///
/// Contains everything required to start a new
/// [`ServiceRunner`](crate::services::runner::ServiceRunner).
pub struct ServiceResources<Message, Settings, State, StateOperator, RuntimeServiceId> {
    // Overwatch
    pub overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    // Status
    pub status_handle: StatusHandle,
    // Settings
    pub settings_updater: SettingsUpdater<Settings>,
    settings_notifier: SettingsNotifier<Settings>,
    // State
    pub state_handle: StateHandle<State, StateOperator>,
    state_updater: StateUpdater<State>,
    operator_fuse_sender: fuse::Sender,
    // Lifecycle
    pub lifecycle_handle: LifecycleHandle,
    // Relay
    pub inbound_relay: Option<InboundRelay<Message>>,
    pub outbound_relay: OutboundRelay<Message>,
    pub consumer_sender: ConsumerSender<Message>,
    pub consumer_receiver: ConsumerReceiver<Message>,
    relay_buffer_size: usize,
}

impl<Message, Settings, State, StateOperator, RuntimeServiceId>
    ServiceResources<Message, Settings, State, StateOperator, RuntimeServiceId>
where
    RuntimeServiceId: Clone,
    Settings: Clone,
    State: ServiceState<Settings = Settings> + Clone,
    StateOperator: StateOperatorTrait<State = State>,
{
    #[must_use]
    pub fn new(
        settings: Settings,
        overwatch_handle: OverwatchHandle<RuntimeServiceId>,
        relay_buffer_size: usize,
    ) -> Self {
        let lifecycle_handle = LifecycleHandle::new();
        let relay = Relay::new(relay_buffer_size);
        let status_handle = StatusHandle::new();
        let state_operator = StateOperator::from_settings(&settings);
        let settings_updater = SettingsUpdater::new(settings);
        let settings_notifier = settings_updater.notifier();

        let (operator_fuse_sender, operator_fuse_receiver) = fuse::channel();
        let (state_handle, state_updater) =
            StateHandle::<State, StateOperator>::new(state_operator, None, operator_fuse_receiver);

        let Relay {
            inbound_relay,
            outbound_relay,
            consumer_sender,
            consumer_receiver,
        } = relay;

        Self {
            overwatch_handle,
            status_handle,
            settings_updater,
            settings_notifier,
            state_handle,
            state_updater,
            operator_fuse_sender,
            lifecycle_handle,
            inbound_relay: Some(inbound_relay),
            outbound_relay,
            consumer_sender,
            consumer_receiver,
            relay_buffer_size,
        }
    }

    /// Create a new [`ServiceResourcesHandle`] from the current
    /// `ServiceResources`.
    ///
    /// # Parameters
    ///
    /// * `inbound_relay`: The relay the service will use to receive messages.
    ///   Due to the singleton nature of the inbound relay, if the recipient
    ///   service is being restarted, then the relay should be the same one
    ///   returned by the previous instance when it was stopped. This ensures
    ///   the new instance will maintain communication with other services who
    ///   opened a relay to the previous instance.
    #[must_use]
    pub fn to_handle(
        &self,
        inbound_relay: InboundRelay<Message>,
    ) -> ServiceResourcesHandle<Message, Settings, State, RuntimeServiceId> {
        ServiceResourcesHandle {
            inbound_relay,
            status_updater: self.status_handle.service_updater().clone(),
            overwatch_handle: self.overwatch_handle.clone(),
            settings_updater: self.settings_updater.clone(),
            state_updater: self.state_updater.clone(),
        }
    }

    pub const fn settings_notifier(&self) -> &SettingsNotifier<Settings> {
        &self.settings_notifier
    }

    pub const fn state_updater(&self) -> &StateUpdater<State> {
        &self.state_updater
    }

    pub const fn relay_buffer_size(&self) -> usize {
        self.relay_buffer_size
    }

    pub const fn operator_fuse_sender(&self) -> &fuse::Sender {
        &self.operator_fuse_sender
    }

    /// Retrieves the inbound relay consumer from the channel.
    ///
    /// Only one inbound relay exists at a time.
    ///
    /// This function must be called only if awaiting a `Service` to be Dropped,
    /// which is when the inbound relay consumer is returned.
    ///
    /// # Errors
    ///
    /// If the inbound relay already exists in [`ServiceResources`].
    ///
    /// # Panics
    ///
    /// If the consumer cannot be retrieved from the channel.
    pub fn retrieve_inbound_relay_consumer(&mut self) -> Result<(), String> {
        if self.inbound_relay.is_some() {
            return Err(String::from("Inbound relay already exists."));
        }
        let inbound_consumer = self
            .consumer_receiver
            .recv()
            .map_err(|error| format!("Failed to receive the InboundRelay consumer: {error}"))?;
        let inbound_relay = InboundRelay::new(
            inbound_consumer,
            self.consumer_sender.clone(),
            self.relay_buffer_size(),
        );
        self.inbound_relay = Some(inbound_relay);
        Ok(())
    }

    /// Retrieves the initial state for the service.
    ///
    /// First tries to load the state from the operator (a previously saved
    /// state). If it fails, it defaults to the initial state created from
    /// the settings.
    ///
    /// # Errors
    ///
    /// If the State fails to load from Settings.
    pub fn get_service_initial_state(&self) -> Result<State, State::Error> {
        let settings = self.settings_notifier.get_updated_settings();
        if let Ok(Some(loaded_state)) = StateOperator::try_load(&settings) {
            info!("Loaded state from Operator");
            Ok(loaded_state)
        } else {
            info!("Couldn't load state from Operator. Creating from settings.");
            State::from_settings(&settings)
        }
    }
}

pub struct ServiceResourcesHandle<Message, Settings, State, RuntimeServiceId> {
    pub inbound_relay: InboundRelay<Message>,
    pub status_updater: StatusUpdater<ServiceAPI>,
    pub overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    pub settings_updater: SettingsUpdater<Settings>,
    pub state_updater: StateUpdater<State>,
}

impl<Message, Settings, State, Operator, RuntimeServiceId>
    From<&ServiceResources<Message, Settings, State, Operator, RuntimeServiceId>>
    for ServiceHandle<Message, Settings, State, Operator>
where
    Settings: Clone,
    State: Clone,
    Operator: Clone,
    RuntimeServiceId: Clone,
{
    fn from(
        service_resources: &ServiceResources<Message, Settings, State, Operator, RuntimeServiceId>,
    ) -> Self {
        Self::new(
            service_resources.outbound_relay.clone(),
            service_resources.settings_updater.clone(),
            service_resources.status_handle.watcher().clone(),
            service_resources.state_handle.clone(),
            service_resources.lifecycle_handle.notifier().clone(),
        )
    }
}
