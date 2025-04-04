use std::time::Duration;

use async_trait::async_trait;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        life_cycle::LifecycleMessage,
        state::{NoOperator, NoState},
        AsServiceId, ServiceCore, ServiceData,
    },
    OpaqueServiceStateHandle,
};
use tokio::time::sleep;
use tokio_stream::StreamExt as _;

pub struct SettingsService {
    state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
}

type SettingsServiceSettings = String;

#[derive(Clone, Debug)]
pub struct SettingsMsg;

impl ServiceData for SettingsService {
    type Settings = SettingsServiceSettings;
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = SettingsMsg;
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for SettingsService {
    fn init(
        state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self { state })
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        let Self {
            state:
                OpaqueServiceStateHandle::<Self, RuntimeServiceId> {
                    settings_reader,
                    lifecycle_handle,
                    ..
                },
        } = self;

        let mut lifecycle_stream = lifecycle_handle.message_stream();

        let lifecycle_message = lifecycle_stream
            .next()
            .await
            .expect("first received message to be a lifecycle message.");

        let sender = match lifecycle_message {
            LifecycleMessage::Shutdown(sender) => {
                println!("Service started 1.");
                if sender.send(()).is_err() {
                    eprintln!(
                        "Error sending successful shutdown signal from service {}",
                        <RuntimeServiceId as AsServiceId<Self>>::SERVICE_ID
                    );
                }
                return Ok(());
            }
            LifecycleMessage::Kill => return Ok(()),
            // Continue below if a `Start` message is received.
            LifecycleMessage::Start(sender) => sender,
        };

        if sender.send(()).is_err() {
            eprintln!(
                "Error sending successful startup signal from service {}",
                <RuntimeServiceId as AsServiceId<Self>>::SERVICE_ID
            );
        }

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

#[derive_services]
struct TestApp {
    settings_service: SettingsService,
}

#[test]
fn settings_service_update_settings() {
    let mut settings: TestAppServiceSettings = TestAppServiceSettings {
        settings_service: SettingsServiceSettings::default(),
    };
    let overwatch = OverwatchRunner::<TestApp>::run(settings.clone(), None).unwrap();
    let handle = overwatch.handle().clone();

    handle
        .runtime()
        .block_on(handle.start_service::<SettingsService>())
        .expect("service to start successfully.");

    let handle2 = handle.clone();
    settings.settings_service = "New settings".to_string();
    overwatch.spawn(async move { handle.clone().update_settings::<TestApp>(settings).await });

    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        handle2.shutdown().await;
    });

    overwatch.wait_finished();
}
