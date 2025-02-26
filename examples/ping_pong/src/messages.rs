// Crate
use overwatch::services::relay::RelayMessage;

#[derive(Debug)]
pub enum PingMessage {
    Pong,
}

impl RelayMessage for PingMessage {}

#[derive(Debug)]
pub enum PongMessage {
    Ping,
}

impl RelayMessage for PongMessage {}
