use overwatch::{overwatch::OverwatchRunner, OpaqueServiceHandle};
use overwatch_derive::Services;

use crate::{service_ping::PingService, service_pong::PongService, settings::PingSettings};

mod messages;
mod operators;
mod service_ping;
mod service_pong;
mod settings;
mod states;

#[derive(Services)]
struct PingPong {
    ping: OpaqueServiceHandle<PingService>,
    pong: OpaqueServiceHandle<PongService>,
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
    ping_pong.wait_finished();
}
