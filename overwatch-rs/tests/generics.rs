use async_trait::async_trait;
use futures::future::select;
use overwatch_derive::Services;
use overwatch_rs::overwatch::OverwatchRunner;
use overwatch_rs::services::handle::{ServiceHandle, ServiceStateHandle};
use overwatch_rs::services::relay::RelayMessage;
use overwatch_rs::services::state::{NoOperator, NoState};
use overwatch_rs::services::{ServiceCore, ServiceData, ServiceId};
use std::fmt::Debug;
use std::time::Duration;
use tokio::time::sleep;

pub struct GenericService<T: Send>
where
    T: Debug + 'static + Sync,
{
    state: ServiceStateHandle<Self>,
    _phantom: std::marker::PhantomData<T>,
}

#[derive(Clone, Debug)]
pub struct GenericServiceMessage(String);

impl RelayMessage for GenericServiceMessage {}

impl<T: Send> ServiceData for GenericService<T>
where
    T: Debug + 'static + Sync,
{
    const SERVICE_ID: ServiceId = "FooService";
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = GenericServiceMessage;
}

#[async_trait]
impl<T: Send> ServiceCore for GenericService<T>
where
    T: Debug + 'static + Sync,
{
    fn init(state: ServiceStateHandle<Self>) -> Result<Self, overwatch_rs::DynError> {
        Ok(Self {
            state,
            _phantom: std::marker::PhantomData,
        })
    }

    async fn run(mut self) -> Result<(), overwatch_rs::DynError> {
        use tokio::io::{self, AsyncWriteExt};

        let Self {
            state: ServiceStateHandle { inbound_relay, .. },
            ..
        } = self;

        let generic = async move {
            let mut stdout = io::stdout();
            let mut inbound_relay = inbound_relay.inner_relay();
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

#[derive(Services)]
struct TestApp<T: Send>
where
    T: Debug + 'static + Sync,
{
    generic_service: ServiceHandle<GenericService<T>>,
}

#[test]
fn derive_generic_service() {
    let settings: TestAppServiceSettings<String> = TestAppServiceSettings {
        generic_service: (),
    };
    let overwatch = OverwatchRunner::<TestApp<String>>::run(settings, None).unwrap();
    let handle = overwatch.handle().clone();
    let generic_service_relay = handle.relay::<GenericService<String>>();

    overwatch.spawn(async move {
        let generic_service_relay = generic_service_relay
            .connect()
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

    overwatch.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        handle.shutdown().await;
    });
    overwatch.wait_finished();
}
