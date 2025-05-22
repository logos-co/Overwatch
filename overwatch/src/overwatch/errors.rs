use thiserror::Error;

pub type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Overwatch base error type.
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Any(DynError),
}

impl From<DynError> for Error {
    fn from(err: DynError) -> Self {
        Self::Any(err)
    }
}
