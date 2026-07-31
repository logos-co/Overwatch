//! Regression test for requesting a relay or a status watcher once Overwatch
//! has shut down.
//!
//! `OverwatchHandle::relay` and `OverwatchHandle::status_watcher` both send a
//! command over an `mpsc` channel whose receiver lives in the Overwatch runner.
//! Once the runner has finished, that receiver is gone and the send fails.

use async_trait::async_trait;
use overwatch::{
    DynError, OpaqueServiceResourcesHandle,
    overwatch::{Error, OverwatchRunner},
    services::{
        ServiceCore, ServiceData,
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

    // The command receiver has been dropped, so the send inside `relay` fails.
    // This must surface as an error rather than panicking.
    let result = handle.relay::<IdleService>().await;
    assert!(
        matches!(result, Err(Error::Dead(_))),
        "relay after shutdown should return Error::Dead, got {result:?}"
    );
}

#[tokio::test]
async fn status_watcher_after_shutdown_returns_error_instead_of_panicking() {
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

    // The command receiver has been dropped, so the send inside `status_watcher`
    // fails. This must surface as an error rather than panicking.
    let result = handle.status_watcher::<IdleService>().await;
    assert!(
        matches!(result, Err(Error::Dead(_))),
        "status_watcher after shutdown should return Error::Dead, got {result:?}"
    );
}
