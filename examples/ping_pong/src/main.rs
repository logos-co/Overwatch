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
    ping_service: PingService,
    pong_service: PongService,
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
        ping_service: ping_settings,
        pong_service: (),
    };
    let ping_pong =
        OverwatchRunner::<PingPong>::run(ping_pong_settings, None).expect("OverwatchRunner failed");
    ping_pong.wait_finished();
}
