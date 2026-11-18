use core::convert::Infallible;

use overwatch::services::state::ServiceState;
use serde::{Deserialize, Serialize};

use crate::settings::PingSettings;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PingState {
    pub pong_count: u32,
}

impl ServiceState for PingState {
    type Settings = PingSettings;
    type Error = Infallible;

    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}
