// Crates
use overwatch::services::state::ServiceState;
use serde::{Deserialize, Serialize};
// Internal
use crate::settings::PingSettings;

#[derive(thiserror::Error, Debug)]
pub enum PingStateError {}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PingState {
    pub pong_count: u32,
}

impl ServiceState for PingState {
    type Settings = PingSettings;
    type Error = PingStateError;

    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}
