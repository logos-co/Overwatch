use async_trait::async_trait;
use overwatch::{
    overwatch::OverwatchRunner,
    services::{
        state::{NoOperator, NoState},
        ServiceCore, ServiceData,
    },
    OpaqueServiceResourcesHandle,
};
use overwatch_derive::derive_services;

pub struct MyService {
    service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

impl ServiceData for MyService {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for MyService {
    fn init(
        service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self {
            service_resources_handle,
        })
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        self.service_resources_handle
            .overwatch_handle
            .shutdown()
            .await
            .expect("Overwatch should shutdown successfully.");
        Ok(())
    }
}

#[derive_services]
struct App {
    my_service: MyService,
}

#[tokio::test]
async fn test_initialisation_from_async_context() {
    let handle = tokio::runtime::Handle::current();
    let settings = AppServiceSettings { my_service: () };
    let app =
        OverwatchRunner::<App>::run(settings, Some(handle)).expect("OverwatchRunner should start.");

    app.handle()
        .start_all_services()
        .await
        .expect("Services should be started successfully.");

    app.wait_finished().await;
}
