// STD
use std::fmt::Debug;
// Crates
use overwatch_rs::services::state::{ServiceState, StateOperator};
// Internal
use crate::states::PingState;

#[derive(Debug, Clone)]
pub struct StateSaveOperator {
    save_path: String,
}

#[async_trait::async_trait]
impl StateOperator for StateSaveOperator {
    type StateInput = PingState;
    type LoadError = std::io::Error;

    fn try_load(
        settings: &<Self::StateInput as ServiceState>::Settings,
    ) -> Result<Option<Self::StateInput>, Self::LoadError> {
        let state_string = std::fs::read_to_string(&settings.state_save_path)?;
        serde_json::from_str(&state_string)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }

    fn from_settings(settings: <Self::StateInput as ServiceState>::Settings) -> Self {
        Self {
            save_path: settings.state_save_path,
        }
    }

    async fn run(&mut self, state: Self::StateInput) {
        let json_state = serde_json::to_string(&state).expect("Failed to serialize state");
        std::fs::write(&self.save_path, json_state).unwrap();
    }
}
