use std::fmt::Debug;

use thiserror::Error;
use tokio::sync::mpsc as tokio_mpsc;

#[derive(Error, Debug)]
pub enum OutboundRelayError<T> {
    #[error("Could not send message to relay: {0}")]
    Send(#[from] tokio_mpsc::error::SendError<T>),
}
