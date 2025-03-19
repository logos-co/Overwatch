pub mod handle;
pub mod life_cycle;
pub mod relay;
pub mod settings;
pub mod state;
pub mod status;

use async_trait::async_trait;
use handle::ServiceStateHandle;
use state::ServiceState;

/// The core data a service needs to handle.
/// Holds the necessary information of a service.
pub trait ServiceData {
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

pub trait ServiceId<T>: ServiceData {
    const SERVICE_ID: T;
}

// This impl is to let services know each other they implement `ServiceId`
// without knowing what the runtime will be, simply based on the
// `RuntimeServiceId`.
impl<RuntimeServiceId, Service> ServiceId<RuntimeServiceId> for Service
where
    RuntimeServiceId: ConvertFrom<Service>,
    Service: ServiceData,
{
    const SERVICE_ID: RuntimeServiceId = RuntimeServiceId::TO;
}

pub trait ConvertFrom<T> {
    const TO: Self;
}

/// Main trait for Services initialization and main loop hook.
#[async_trait]
pub trait ServiceCore<RuntimeServiceId>: Sized + ServiceData {
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
            RuntimeServiceId,
        >,
        initial_state: Self::State,
    ) -> Result<Self, super::DynError>;

    /// Main loop
    async fn run(mut self) -> Result<(), super::DynError>;
}
