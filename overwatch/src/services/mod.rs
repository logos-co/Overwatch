pub mod handle;
pub mod life_cycle;
pub mod relay;
pub mod settings;
pub mod state;
pub mod status;

use async_trait::async_trait;
use handle::ServiceStateHandle;
use state::ServiceState;

// TODO: Make this type unique for each service?
/// Services identification type.
pub type ServiceId = &'static str;

/// The core data a service needs to handle.
/// Holds the necessary information of a service.
pub trait ServiceData {
    /// Service identification tag
    const SERVICE_ID: ServiceId;
    /// Service relay buffer size
    const SERVICE_RELAY_BUFFER_SIZE: usize = 16;
    /// Service settings object
    type Settings;
    /// Service state object
    type State;
    /// State operator
    type StateOperator;
    /// Service messages that the service itself understands and can react to
    type Message;
}

/// Main trait for Services initialization and main loop hook.
#[async_trait]
pub trait ServiceCore<AggregatedServiceId>: Sized + ServiceData {
    /// Initialize the service with the given handle and initial state.
    ///
    /// # Errors
    ///
    /// The initialization creation error.
    fn init(
        service_state_handle: ServiceStateHandle<
            Self::Message,
            Self::Settings,
            Self::State,
            AggregatedServiceId,
        >,
        initial_state: Self::State,
    ) -> Result<Self, super::DynError>;

    /// Main loop
    async fn run(mut self) -> Result<(), super::DynError>;
}
