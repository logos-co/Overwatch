use std::fmt::Debug;

use overwatch::services::state::{ServiceState, StateOperator};

use crate::states::PingState;

#[derive(Debug, Clone)]
pub struct StateSaveOperator {
    save_path: String,
}

#[async_trait::async_trait]
impl StateOperator for StateSaveOperator {
    type State = PingState;
    type LoadError = std::io::Error;

    fn try_load(
        settings: &<Self::State as ServiceState>::Settings,
    ) -> Result<Option<Self::State>, Self::LoadError> {
        let state_string = std::fs::read_to_string(&settings.state_save_path)?;
        serde_json::from_str(&state_string)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }

    fn from_settings(settings: &<Self::State as ServiceState>::Settings) -> Self {
        Self {
            save_path: settings.state_save_path.clone(),
        }
    }

    async fn run(&mut self, state: Self::State) {
        let json_state = serde_json::to_string(&state).expect("Failed to serialize state");
        std::fs::write(&self.save_path, json_state).unwrap();
    }
}
