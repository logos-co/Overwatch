use std::fmt::Debug;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayError {
    #[error("couldn't relay message")]
    Send,
    #[error("receiver failed due to {0:?}")]
    Receiver(Box<dyn Debug + Send + Sync>),
}
