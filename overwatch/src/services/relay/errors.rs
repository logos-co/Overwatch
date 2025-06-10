use std::fmt::Debug;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayError {
    #[error("couldn't relay message")]
    Send,
    #[error("relay is already connected")]
    AlreadyConnected,
    #[error("receiver failed due to {0:?}")]
    Receiver(Box<dyn Debug + Send + Sync>),
}

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Couldn't start service")]
    Start,
    #[error("Couldn't stop service")]
    Stop,
}

#[derive(Error, Debug)]
pub enum OverwatchError {
    #[error("C")]
    Service(ServiceError),
    #[error("Couldn't shutdown Overwatch")]
    Shutdown,
}
