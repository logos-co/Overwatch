use std::time::Duration;

use async_trait::async_trait;
use futures::future::select;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        life_cycle::LifecycleMessage,
        state::{NoOperator, NoState},
        ServiceCore, ServiceData,
    },
    OpaqueServiceStateHandle,
};
use tokio::time::sleep;
use tokio_stream::StreamExt as _;

pub struct PrintService {
    state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
}

#[derive(Clone, Debug)]
pub struct PrintServiceMessage(String);

impl ServiceData for PrintService {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = PrintServiceMessage;
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for PrintService {
    fn init(
        state: OpaqueServiceStateHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self { state })
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        use tokio::io::{self, AsyncWriteExt};

        let Self {
            state:
                OpaqueServiceStateHandle::<Self, RuntimeServiceId> {
                    mut inbound_relay,
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
                sender.send(()).unwrap();
                return Ok(());
            }
            LifecycleMessage::Kill => return Ok(()),
            // Continue below if a `Start` message is received.
            LifecycleMessage::Start(sender) => sender,
        };

        sender.send(()).unwrap();

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

    handle
        .runtime()
        .block_on(handle.start_service::<PrintService>())
        .expect("service to start successfully.");

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
