use std::str::FromStr as _;

use async_trait::async_trait;
use overwatch::{
    OpaqueServiceResourcesHandle,
    services::{
        ServiceCore, ServiceData,
        state::{NoOperator, NoState},
    },
};
use overwatch_derive::derive_services;

struct MyService;

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

struct OtherService;

impl ServiceData for OtherService {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for OtherService {
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
    my_service: MyService,             // Variant name follows the service name
    alternative_service: OtherService, // Variant name is different from the service name
}

#[test]
fn from_str() {
    let runtime_service_id =
        RuntimeServiceId::from_str("MyService").expect("Should parse service ID.");
    assert_eq!(runtime_service_id, RuntimeServiceId::MyService);

    let alternative_service_id =
        RuntimeServiceId::from_str("AlternativeService").expect("Should parse service ID.");
    assert_eq!(alternative_service_id, RuntimeServiceId::AlternativeService);

    RuntimeServiceId::from_str("non-existing-service").expect_err("Should fail.");
}
