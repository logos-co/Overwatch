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
    println!("Starting overwatch service");
    ping_pong
        .runtime()
        .handle()
        .block_on(overwatch_handle.start_all_services())
        .expect("Error starting overwatch service");

    ping_pong.blocking_wait_finished();
}
