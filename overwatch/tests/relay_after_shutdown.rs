//! Regression test for requesting a relay once Overwatch has shut down.
//!
//! `OverwatchHandle::relay` sends a command over an mpsc channel whose receiver
//! lives in the Overwatch runner. Once the runner has finished, that receiver is
//! gone and the send fails. That path used to hit an `unreachable!`, so a relay
//! requested concurrently with shutdown panicked instead of returning the
//! `RelayError` the function already advertises.

use async_trait::async_trait;
use overwatch::{
    DynError, OpaqueServiceResourcesHandle,
    overwatch::OverwatchRunner,
    services::{
        ServiceCore, ServiceData,
        relay::RelayError,
        state::{NoOperator, NoState},
    },
};
use overwatch_derive::derive_services;

pub struct IdleService;

impl ServiceData for IdleService {
    type Settings = ();
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for IdleService {
    fn init(
        _service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self)
    }

    async fn run(self) -> Result<(), DynError> {
        Ok(())
    }
}

#[derive_services]
struct App {
    idle_service: IdleService,
}

#[tokio::test]
async fn relay_after_shutdown_returns_error_instead_of_panicking() {
    let runtime_handle = tokio::runtime::Handle::current();
    let settings = AppServiceSettings { idle_service: () };
    let app = OverwatchRunner::<App>::run(settings, Some(runtime_handle))
        .expect("OverwatchRunner should start.");

    // Keep a handle alive past the runner, exactly as a long-lived caller
    // (an HTTP handler, an FFI binding) would hold one.
    let handle = app.handle().clone();

    handle
        .shutdown()
        .await
        .expect("Overwatch should shut down successfully.");
    app.wait_finished().await;

    // The command receiver is dropped by now, so the send inside `relay` fails.
    // This must surface as an error rather than panicking.
    let result = handle.relay::<IdleService>().await;

    assert!(
        matches!(result, Err(RelayError::Send)),
        "relay after shutdown should return RelayError::Send"
    );
}
