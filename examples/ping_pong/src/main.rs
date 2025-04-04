#![expect(clippy::similar_names, reason = "Test services.")]

use futures::future::join;
use overwatch::{derive_services, overwatch::OverwatchRunner};

use crate::{service_ping::PingService, service_pong::PongService, settings::PingSettings};

mod messages;
mod operators;
mod service_ping;
mod service_pong;
mod settings;
mod states;

#[derive_services]
struct PingPong {
    ping: PingService,
    pong: PongService,
}

const PING_STATE_SAVE_PATH: &str = const_format::formatcp!(
    "{}/saved_states/ping_state.json",
    env!("CARGO_MANIFEST_DIR")
);

fn main() {
    let ping_settings = PingSettings {
        state_save_path: String::from(PING_STATE_SAVE_PATH),
    };
    let ping_pong_settings = PingPongServiceSettings {
        ping: ping_settings,
        pong: (),
    };
    let ping_pong =
        OverwatchRunner::<PingPong>::run(ping_pong_settings, None).expect("OverwatchRunner failed");

    let overwatch_handle = ping_pong.handle().clone();
    let (ping, pong) = ping_pong.runtime().block_on(join(
        overwatch_handle.start_service::<PingService>(),
        overwatch_handle.start_service::<PongService>(),
    ));
    ping.expect("Ping service to start successfully.");
    pong.expect("Pong service to start successfully.");

    ping_pong.wait_finished();
}
