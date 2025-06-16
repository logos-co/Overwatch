use std::str::FromStr;

use async_trait::async_trait;
use overwatch::{
    services::{
        state::{NoOperator, NoState},
        ServiceCore, ServiceData,
    },
    OpaqueServiceResourcesHandle,
};
use overwatch_derive::derive_services;

pub struct MyService;

impl ServiceData for MyService {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for MyService {
    fn init(
        _service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, overwatch::DynError> {
        Ok(Self {})
    }

    async fn run(mut self) -> Result<(), overwatch::DynError> {
        Ok(())
    }
}

#[derive_services]
struct App {
    my_service: MyService,
}

#[test]
fn from_str() {
    let runtime_service_id =
        RuntimeServiceId::from_str("my_service").expect("Should parse service ID.");
    assert_eq!(runtime_service_id, RuntimeServiceId::MyService);

    RuntimeServiceId::from_str("non-existing-service").expect_err("Should fail.");
}
