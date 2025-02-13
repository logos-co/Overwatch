use async_trait::async_trait;
use futures::future::select;
use overwatch_derive::Services;
use overwatch_rs::overwatch::OverwatchRunner;
use overwatch_rs::services::relay::RelayMessage;
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use overwatch_rs::{OpaqueServiceHandle, OpaqueServiceStateHandle};
use std::time::Duration;
use tokio::time::sleep;

pub struct PrintService {
    state: OpaqueServiceStateHandle<Self>,
}

#[derive(Clone, Debug)]
pub struct PrintServiceMessage(String);

impl RelayMessage for PrintServiceMessage {}

impl ServiceData for PrintService {
    const SERVICE_ID: ServiceId = "FooService";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State, Self::Settings>;
    type Message = PrintServiceMessage;
}

#[async_trait]
impl ServiceCore for PrintService {
    fn init(
        state: OpaqueServiceStateHandle<Self>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch_rs::DynError> {
        Ok(Self { state })
    }

    async fn run(mut self) -> Result<(), overwatch_rs::DynError> {
        use tokio::io::{self, AsyncWriteExt};

        let Self {
            state: OpaqueServiceStateHandle {
                mut inbound_relay, ..
            },
        } = self;

        let print = async move {
            let mut stdout = io::stdout();
            while let Some(message) = inbound_relay.recv().await {
                match message.0.as_ref() {
                    "stop" => {
                        stdout
                            .write_all(b"Printing service stopping\n")
                            .await
                            .expect("stop Output wrote");
                        break;
                    }
                    m => {
                        stdout
                            .write_all(format!("{m}\n").as_bytes())
                            .await
                            .expect("Message output wrote");
                    }
                }
            }
        };

        let idle = async move {
            let mut stdout = io::stdout();
            loop {
                stdout
                    .write_all(b"Waiting for print process to finish...\n")
                    .await
                    .expect("Message output wrote");
                sleep(Duration::from_millis(50)).await;
            }
        };

        select(Box::pin(print), Box::pin(idle)).await;
        Ok(())
    }
}

#[derive(Services)]
struct TestApp {
    print_service: OpaqueServiceHandle<PrintService>,
}

#[test]
fn derive_print_service() {
    let settings: TestAppServiceSettings = TestAppServiceSettings { print_service: () };
    let overwatch = OverwatchRunner::<TestApp>::run(settings, None).unwrap();
    let handle = overwatch.handle().clone();
    let print_service_relay = handle.relay::<PrintService>();

    overwatch.spawn(async move {
        let print_service_relay = print_service_relay
            .connect()
            .await
            .expect("A connection to the print service is established");

        for _ in 0..3 {
            print_service_relay
                .send(PrintServiceMessage("Hey oh let's go!".to_string()))
                .await
                .expect("Message is sent");
        }
        sleep(Duration::from_millis(50)).await;
        print_service_relay
            .send(PrintServiceMessage("stop".to_string()))
            .await
            .expect("stop message to be sent");
    });

    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        handle.shutdown().await;
    });
    overwatch.wait_finished();
}
