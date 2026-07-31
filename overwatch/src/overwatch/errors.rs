use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

use crate::services::lifecycle::ServiceLifecycleError;

pub type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Overwatch base error type.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Overwatch is dead: {0}")]
    Dead(#[source] DynError),
    #[error(transparent)]
    Any(DynError),
}

pub fn send_error_to_dead<T>(error: &SendError<T>) -> Error {
    // As long as `AnySettings` remains non-`Sync`, this error cannot be `Sync`
    // either. Thus, the KISS solution we decided for is to stringify it, since
    // the underlying error is not as important as the fact that it is terminal
    // and cannot be retried.
    Error::Dead(format!("{error:?}").into())
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
