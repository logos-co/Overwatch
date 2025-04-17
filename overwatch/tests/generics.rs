use std::{fmt::Debug, time::Duration};

use async_trait::async_trait;
use futures::future::select;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        life_cycle::LifecycleMessage,
        state::{NoOperator, NoState},
        state_handle::ServiceStateHandle,
        ServiceCore, ServiceData,
    },
};
use tokio::time::sleep;
use tokio_stream::StreamExt as _;

pub struct GenericService {
    state: ServiceStateHandle<GenericServiceMessage, (), NoState<()>, RuntimeServiceId>,
}

#[derive(Clone, Debug)]
pub struct GenericServiceMessage(String);

impl ServiceData for GenericService {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = GenericServiceMessage;
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for GenericService {
    fn init(
        state: ServiceStateHandle<Self::Message, Self::Settings, Self::State, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self { state })
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        use tokio::io::{self, AsyncWriteExt};

        let Self {
            state:
                ServiceStateHandle {
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

        let generic = async move {
            let mut stdout = io::stdout();
            while let Some(message) = inbound_relay.recv().await {
                match message.0.as_ref() {
                    "stop" => {
                        stdout
                            .write_all(b"genericing service stopping\n")
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
                    .write_all(b"Waiting for generic process to finish...\n")
                    .await
                    .expect("Message output wrote");
                sleep(Duration::from_millis(50)).await;
            }
        };

        select(Box::pin(generic), Box::pin(idle)).await;
        Ok(())
    }
}

#[derive_services]
struct TestApp {
    generic_service: GenericService,
}

#[test]
fn derive_generic_service() {
    let settings: TestAppServiceSettings = TestAppServiceSettings {
        generic_service: (),
    };
    let overwatch = OverwatchRunner::<TestApp>::run(settings, None).unwrap();
    let handle = overwatch.handle().clone();

    handle
        .runtime()
        .block_on(handle.start_service::<GenericService>())
        .expect("service to start successfully.");

    overwatch.spawn(async move {
        let generic_service_relay = handle
            .relay::<GenericService>()
            .await
            .expect("A connection to the generic service is established");

        for _ in 0..3 {
            generic_service_relay
                .send(GenericServiceMessage("Hey oh let's go!".to_string()))
                .await
                .expect("Message is sent");
        }
        sleep(Duration::from_millis(50)).await;
        generic_service_relay
            .send(GenericServiceMessage("stop".to_string()))
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
