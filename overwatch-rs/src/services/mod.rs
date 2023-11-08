pub mod handle;
pub mod life_cycle;
pub mod relay;
pub mod settings;
pub mod state;

// std
use std::fmt::{Debug, Display};
// crates
use async_trait::async_trait;
use thiserror::Error;
use tokio::runtime;

// internal
use crate::services::relay::RelayError;
use crate::services::state::StateOperator;
use handle::ServiceStateHandle;
use relay::RelayMessage;
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
    type Settings: Clone;
    /// Service state object
    type State: ServiceState<Settings = Self::Settings> + Clone;
    /// State operator
    type StateOperator: StateOperator<StateInput = Self::State> + Clone;
    /// Service messages that the service itself understands and can react to
    type Message: RelayMessage + Debug;
}

/// Main trait for Services initialization and main loop hook
#[async_trait]
pub trait ServiceCore: Sized + ServiceData {
    /// Initialize the service with the given state
    fn init(service_state: ServiceStateHandle<Self>) -> Result<Self, ServiceError>;

    /// Service main loop
    async fn run(mut self) -> Result<(), ServiceError>;
}

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error(transparent)]
    RelayError(#[from] RelayError),

    #[error("{0}'s notifier closed")]
    NotifierClosed(ServiceId),

    #[error(transparent)]
    Service(Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("{0}")]
    Custom(String),
}

impl ServiceError {
    pub fn service(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Service(Box::new(err))
    }

    pub fn custom(err: impl Display) -> Self {
        Self::Custom(err.to_string())
    }
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
            _ => None,
        }
    }
}
