use overwatch::{
    overwatch::OverwatchRunner,
    services::{
        state::{NoOperator, NoState},
        ServiceCore, ServiceData,
    },
    DynError, OpaqueServiceResourcesHandle,
};
use overwatch_derive::derive_services;

#[derive(Clone, Debug)]
struct OnStopSettings {
    is_alive_sender: std::sync::mpsc::Sender<()>,
    on_stop_sender: std::sync::mpsc::Sender<()>,
}

struct OnStopService {
    is_alive_sender: std::sync::mpsc::Sender<()>,
    on_stop_sender: std::sync::mpsc::Sender<()>,
}

impl ServiceData for OnStopService {
    type Settings = OnStopSettings;
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    type Message = ();
}

#[async_trait::async_trait]
impl ServiceCore<RuntimeServiceId> for OnStopService {
    fn init(
        service_resources_handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _initial_state: Self::State,
    ) -> Result<Self, DynError> {
        let settings = service_resources_handle
            .settings_handle
            .notifier()
            .get_updated_settings();
        Ok(Self {
            is_alive_sender: settings.is_alive_sender,
            on_stop_sender: settings.on_stop_sender,
        })
    }

    async fn run(mut self) -> Result<(), DynError> {
        self.is_alive_sender
            .send(())
            .expect("Failed to send is_alive signal");
        std::future::pending::<()>().await;
        Ok(())
    }
}

impl Drop for OnStopService {
    fn drop(&mut self) {
        self.on_stop_sender
            .send(())
            .expect("Failed to send on_stop signal");
    }
}

#[derive_services]
struct App {
    on_stop_service: OnStopService,
}

#[test]
fn on_stop() {
    let (is_alive_sender, is_alive_receiver) = std::sync::mpsc::channel();
    let (on_stop_sender, on_stop_receiver) = std::sync::mpsc::channel();
    let settings = AppServiceSettings {
        on_stop_service: OnStopSettings {
            is_alive_sender,
            on_stop_sender,
        },
    };
    let overwatch = OverwatchRunner::<App>::run(settings, None).expect("Failed to run overwatch");

    let handle = overwatch.handle();
    let runtime = handle.runtime();
    let _ = runtime.block_on(handle.start_all_services());

    // Wait until the service is alive to stop it
    is_alive_receiver
        .recv()
        .expect("Failed to receive the is_alive signal");

    // Stop all services
    let _ = runtime.block_on(handle.stop_all_services());

    // Wait for the on_stop signal (sent from `Drop`)
    on_stop_receiver
        .recv()
        .expect("Failed to receive the on_stop signal");

    let _ = runtime.block_on(handle.shutdown());
    overwatch.wait_finished();
}
