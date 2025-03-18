use std::time::Duration;

use async_trait::async_trait;
use futures::future::select;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        state::{NoOperator, NoState},
        ServiceCore, ServiceData, ServiceId,
    },
    OpaqueServiceStateHandle,
};
use tokio::time::sleep;

pub struct PrintService {
    state: OpaqueServiceStateHandle<Self, AggregatedServiceId>,
}

#[derive(Clone, Debug)]
pub struct PrintServiceMessage(String);

impl ServiceData for PrintService {
    const SERVICE_ID: ServiceId = "FooService";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = PrintServiceMessage;
}

#[async_trait]
impl ServiceCore<AggregatedServiceId> for PrintService {
    fn init(
        state: OpaqueServiceStateHandle<Self, AggregatedServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self { state })
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        use tokio::io::{self, AsyncWriteExt};

        let Self {
            state:
                OpaqueServiceStateHandle::<Self, AggregatedServiceId> {
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

#[derive_services]
struct TestApp {
    print_service: PrintService,
}

#[test]
fn derive_print_service() {
    let settings: TestAppServiceSettings = TestAppServiceSettings { print_service: () };
    let overwatch = OverwatchRunner::<TestApp>::run(settings, None).unwrap();
    let handle = overwatch.handle().clone();

    overwatch.spawn(async move {
        let print_service_relay = handle
            .relay::<PrintService>()
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

    let handle = overwatch.handle().clone();
    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        handle.shutdown().await;
    });
    overwatch.wait_finished();
}
