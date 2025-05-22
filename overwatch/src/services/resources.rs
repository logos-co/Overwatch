use tracing::info;

use crate::{
    overwatch::handle::OverwatchHandle,
    services::{
        handle::ServiceHandle,
        life_cycle::LifecycleHandle,
        relay::{InboundRelay, InboundRelayReceiver, InboundRelaySender, OutboundRelay, Relay},
        settings::SettingsHandle,
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
    overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    // Status
    status_handle: StatusHandle,
    // Settings
    settings_handle: SettingsHandle<Settings>,
    // State
    state_handle: StateHandle<State, StateOperator>,
    state_updater: StateUpdater<State>,
    operator_fuse_sender: fuse::Sender,
    // Lifecycle
    lifecycle_handle: LifecycleHandle,
    // Relay
    inbound_relay: Option<InboundRelay<Message>>,
    outbound_relay: OutboundRelay<Message>,
    inbound_relay_sender: InboundRelaySender<Message>,
    inbound_relay_receiver: InboundRelayReceiver<Message>,
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
        let settings_handle = SettingsHandle::new(settings);

        let (operator_fuse_sender, operator_fuse_receiver) = fuse::channel();
        let (state_handle, state_updater) =
            StateHandle::<State, StateOperator>::new(state_operator, None, operator_fuse_receiver);

        let Relay {
            inbound_relay,
            outbound_relay,
            inbound_relay_sender,
            inbound_relay_receiver,
        } = relay;

        Self {
            overwatch_handle,
            status_handle,
            settings_handle,
            state_handle,
            state_updater,
            operator_fuse_sender,
            lifecycle_handle,
            inbound_relay: Some(inbound_relay),
            outbound_relay,
            inbound_relay_sender,
            inbound_relay_receiver,
            relay_buffer_size,
        }
    }

    pub const fn overwatch_handle(&self) -> &OverwatchHandle<RuntimeServiceId> {
        &self.overwatch_handle
    }

    pub const fn status_handle(&self) -> &StatusHandle {
        &self.status_handle
    }

    pub const fn settings_handle(&self) -> &SettingsHandle<Settings> {
        &self.settings_handle
    }

    pub const fn state_handle(&self) -> &StateHandle<State, StateOperator> {
        &self.state_handle
    }

    pub const fn state_updater(&self) -> &StateUpdater<State> {
        &self.state_updater
    }

    pub const fn lifecycle_handle(&self) -> &LifecycleHandle {
        &self.lifecycle_handle
    }

    pub const fn lifecycle_handle_mut(&mut self) -> &mut LifecycleHandle {
        &mut self.lifecycle_handle
    }

    pub const fn relay_buffer_size(&self) -> usize {
        self.relay_buffer_size
    }

    pub const fn operator_fuse_sender(&self) -> &fuse::Sender {
        &self.operator_fuse_sender
    }

    /// Retrieves the [`InboundRelay`]'s receiver from the channel and rebuilds
    /// a new [`InboundRelay`].
    ///
    /// Only one [`InboundRelay`] exists at a time for a given `Service`.
    ///
    /// This function must be called only if awaiting a `Service` to be Dropped,
    /// which is when the [`InboundRelay`]'s receiver is returned.
    ///
    /// # Errors
    ///
    /// If the [`InboundRelay`] already exists in [`ServiceResources`].
    ///
    /// # Panics
    ///
    /// If the [`InboundRelay`]'s receiver cannot be retrieved from the channel.
    pub fn rebuild_inbound_relay(&mut self) -> Result<(), String> {
        if self.inbound_relay.is_some() {
            return Err(String::from("Inbound relay already exists."));
        }
        let inbound_relay_receiver = self
            .inbound_relay_receiver
            .recv()
            .map_err(|error| format!("Failed to retrieve the InboundRelay's receiver: {error}"))?;
        let inbound_relay = InboundRelay::new(
            inbound_relay_receiver,
            self.inbound_relay_sender.clone(),
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
        let settings = self.settings_handle.notifier().get_updated_settings();
        if let Ok(Some(loaded_state)) = StateOperator::try_load(&settings) {
            info!("Loaded state from Operator");
            Ok(loaded_state)
        } else {
            info!("Couldn't load state from Operator. Creating from settings.");
            State::from_settings(&settings)
        }
    }

    /// Create a new [`ServiceResourcesHandle`](ServiceResourcesHandle) from the
    /// current `ServiceResources`.
    ///
    /// It requires `inbound_relay` to be set.
    ///
    /// # Errors
    ///
    /// If the [`InboundRelay`] is not set in the `ServiceResources`.
    pub fn as_handle(
        &mut self,
    ) -> Result<ServiceResourcesHandle<Message, Settings, State, RuntimeServiceId>, String> {
        let inbound_relay = self
            .inbound_relay
            .take()
            .ok_or_else(|| String::from("InboundRelay is not set in the ServiceResources."))?;

        Ok(ServiceResourcesHandle {
            inbound_relay,
            status_updater: self.status_handle.service_updater().clone(),
            overwatch_handle: self.overwatch_handle.clone(),
            settings_handle: self.settings_handle.clone(),
            state_updater: self.state_updater.clone(),
        })
    }
}

pub struct ServiceResourcesHandle<Message, Settings, State, RuntimeServiceId> {
    pub inbound_relay: InboundRelay<Message>,
    pub status_updater: StatusUpdater<ServiceAPI>,
    pub overwatch_handle: OverwatchHandle<RuntimeServiceId>,
    pub settings_handle: SettingsHandle<Settings>,
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
            service_resources.settings_handle.updater().clone(),
            service_resources.status_handle.watcher().clone(),
            service_resources.state_handle.clone(),
            service_resources.lifecycle_handle.notifier().clone(),
        )
    }
}
