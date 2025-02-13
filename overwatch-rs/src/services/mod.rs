pub mod handle;
pub mod life_cycle;
pub mod relay;
pub mod settings;
pub mod state;
pub mod status;

// std
use std::fmt::Debug;
// crates
use async_trait::async_trait;
use thiserror::Error;
use tokio::runtime;

// internal
use crate::services::relay::RelayError;
use handle::ServiceStateHandle;
use state::ServiceState;

// TODO: Make this type unique for each service?
/// Services identification type
pub type ServiceId = &'static str;

/// The core data a service needs to handle
/// Holds the necessary information of a service
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

/// Main trait for Services initialization and main loop hook
#[async_trait]
pub trait ServiceCore: Sized + ServiceData {
    /// Initialize the service with the given state
    fn init(
        service_state: ServiceStateHandle<Self::Message, Self::Settings, Self, Self::State>,
        initial_state: Self::State,
    ) -> Result<Self, super::DynError>;

    /// Service main loop
    async fn run(mut self) -> Result<(), super::DynError>;
}

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error(transparent)]
    RelayError(#[from] RelayError),
}

pub enum ServiceRuntime {
    FromParent(runtime::Handle),
    Custom(runtime::Runtime),
}

impl ServiceRuntime {
    pub fn handle(&self) -> runtime::Handle {
        match self {
            ServiceRuntime::FromParent(handle) => handle.clone(),
            ServiceRuntime::Custom(runtime) => runtime.handle().clone(),
        }
    }

    pub fn runtime(self) -> Option<runtime::Runtime> {
        match self {
            ServiceRuntime::Custom(runtime) => Some(runtime),
            ServiceRuntime::FromParent(_) => None,
        }
    }
}
