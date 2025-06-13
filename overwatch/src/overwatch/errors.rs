use thiserror::Error;

use crate::services::lifecycle::ServiceLifecycleError;

pub type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Overwatch base error type.
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Any(DynError),
}

#[derive(Error, Debug)]
pub enum OverwatchManagementError {
    #[error("Failed retrieving service ids")]
    RetrieveServiceIds,
    #[error("Failed to shut down Overwatch")]
    Shutdown,
}

impl From<DynError> for Error {
    fn from(err: DynError) -> Self {
        Self::Any(err)
    }
}

impl From<ServiceLifecycleError> for Error {
    fn from(error: ServiceLifecycleError) -> Self {
        Self::Any(error.into())
    }
}

impl From<OverwatchManagementError> for Error {
    fn from(error: OverwatchManagementError) -> Self {
        Self::Any(error.into())
    }
}
