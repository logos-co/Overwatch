use async_trait::async_trait;
use overwatch_derive::Services;
use overwatch_rs::overwatch::handle::OverwatchHandler;
use overwatch_rs::overwatch::OverwatchRunner;
use overwatch_rs::services::handle::{ServiceHandle, ServiceStateHandle};
use overwatch_rs::services::relay::RelayMessage;
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use std::time::Duration;
use tokio::time::sleep;

pub struct SettingsService {
    state: ServiceStateHandle<Self>,
}

type SettingsServiceSettings = String;

#[derive(Clone, Debug)]
pub struct SettingsMsg;

impl RelayMessage for SettingsMsg {}

impl ServiceData for SettingsService {
    const SERVICE_ID: ServiceId = "FooService";
    type Settings = SettingsServiceSettings;
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = SettingsMsg;
}

#[async_trait]
impl ServiceCore for SettingsService {
    fn init(state: ServiceStateHandle<Self>) -> Result<Self, overwatch_rs::DynError> {
        Ok(Self { state })
    }

    async fn run(mut self) -> Result<(), overwatch_rs::DynError> {
        let Self {
            state: ServiceStateHandle {
                settings_reader, ..
            },
        } = self;

        let print = async move {
            let mut asserted = false;
            for _ in 0..10 {
                let new_settings = settings_reader.get_updated_settings();
                if new_settings.as_str() == "New settings" {
                    asserted = true;
                }
                sleep(Duration::from_millis(50)).await;
            }
            // TODO: when [this](https://github.com/ockam-network/ockam/issues/2479)
            // or (https://github.com/tokio-rs/tokio/issues/2002) lands
            // update so this panic is not just a print and the test get actually aborted
            assert!(asserted);
        };
        print.await;
        Ok(())
    }
}

#[derive(Services)]
struct TestApp {
    settings_service: ServiceHandle<SettingsService>,
}

#[test]
fn settings_service_update_settings() {
    let mut settings: TestAppServiceSettings = TestAppServiceSettings {
        settings_service: SettingsServiceSettings::default(),
    };
    let overwatch = OverwatchRunner::<TestApp>::run(settings.clone(), None).unwrap();
    let handle = overwatch.handle().clone();
    let handle2 = handle.clone();
    settings.settings_service = "New settings".to_string();
    overwatch.spawn(async move { handle.clone().update_settings::<TestApp>(settings).await });

    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        handle2.shutdown().await;
    });

    overwatch.wait_finished();
}
