use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServiceLifecycleError {
    #[error("Couldn't start service")]
    Start,
    #[error("Couldn't start the sequence of services")]
    StartSequence,
    #[error("Couldn't start all services")]
    StartAll,
    #[error("Couldn't stop service")]
    Stop,
    #[error("Couldn't stop the sequence of services")]
    StopSequence,
    #[error("Couldn't stop all services")]
    StopAll,
}
