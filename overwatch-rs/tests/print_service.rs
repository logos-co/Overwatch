use async_trait::async_trait;
use futures::future::select;
use overwatch_rs::overwatch::OverwatchRunner;
use overwatch_rs::services::handle::{ServiceHandle, ServiceStateHandle};
use overwatch_rs::services::relay::RelayMessage;
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use overwatch_derive::Services;
use std::time::Duration;
use tokio::time::sleep;

pub struct PrintService {
    state: ServiceStateHandle<Self>,
}

#[derive(Clone, Debug)]
pub struct PrintServiceMessage(String);

impl RelayMessage for PrintServiceMessage {}

impl ServiceData for PrintService {
    const SERVICE_ID: ServiceId = "FooService";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = PrintServiceMessage;
}

#[async_trait]
impl ServiceCore for PrintService {
    fn init(state: ServiceStateHandle<Self>) -> Self {
        Self { state }
    }

    async fn run(mut self) {
        use tokio::io::{self, AsyncWriteExt};

        let Self {
            state: ServiceStateHandle {
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
    }
}

#[derive(Services)]
struct TestApp {
    print_service: ServiceHandle<PrintService>,
}

#[test]
fn derive_print_service() {
    let settings: TestAppServiceSettings = TestAppServiceSettings { print_service: () };
    let overwatch = OverwatchRunner::<TestApp>::run(settings, None);
    let mut handle = overwatch.handle().clone();
    let mut print_service_relay = handle.relay::<PrintService>();

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
