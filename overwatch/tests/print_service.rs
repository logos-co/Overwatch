use std::time::Duration;

use async_trait::async_trait;
use futures::future::select;
use overwatch::{
    OpaqueServiceResourcesHandle, derive_services,
    overwatch::OverwatchRunner,
    services::{
        ServiceCore, ServiceData,
        state::{NoOperator, NoState},
    },
};
use tokio::{
    io::{self, AsyncWriteExt as _},
    time::sleep,
};

pub struct PrintService {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
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
        service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self {
            service_resources_handle,
        })
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        let mut inbound_relay = self.service_resources_handle.inbound_relay;
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
                .send(PrintServiceMessage("Hey oh let's go!".to_owned()))
                .await
                .expect("Message is sent");
        }
        sleep(Duration::from_millis(50)).await;
        print_service_relay
            .send(PrintServiceMessage("stop".to_owned()))
            .await
            .expect("stop message to be sent");
    });

    let handle = overwatch.handle().clone();
    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        let _ = handle.shutdown().await;
    });
    overwatch.blocking_wait_finished();
}
