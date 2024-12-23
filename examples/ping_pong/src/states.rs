// STD
use std::io;
// Crates
use overwatch_rs::services::state::ServiceState;
use serde::{Deserialize, Serialize};
// Internal
use crate::settings::PingSettings;

#[derive(thiserror::Error, Debug)]
pub enum PingStateError {}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PingState {
    pub pong_count: u32,
}

impl PingState {
    fn load_saved_state(save_path: &str) -> io::Result<Self> {
        let json_state = std::fs::read(save_path)?;
        let state = serde_json::from_slice(json_state.as_slice())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        Ok(state)
    }
}

impl ServiceState for PingState {
    type Settings = PingSettings;
    type Error = PingStateError;

    fn from_settings(settings: &Self::Settings) -> Result<Self, Self::Error> {
        let state = Self::load_saved_state(settings.state_save_path.as_str())
            .unwrap_or_else(|_error| Self::default());
        Ok(state)
    }
}
