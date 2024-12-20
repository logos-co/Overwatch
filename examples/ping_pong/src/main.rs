// Crate
use overwatch_derive::Services;
use overwatch_rs::overwatch::OverwatchRunner;
use overwatch_rs::services::handle::ServiceHandle;
// Internal
use crate::service_ping::PingService;
use crate::service_pong::PongService;

mod service_ping;
mod service_pong;
mod messages;

#[derive(Services)]
struct PingPong {
    ping: ServiceHandle<PingService>,
    pong: ServiceHandle<PongService>,
}

fn main() {
    let ping_pong_settings = PingPongServiceSettings {
        ping: (),
        pong: (),
    };
    let ping_pong = OverwatchRunner::<PingPong>::run(ping_pong_settings, None).expect("OverwatchRunner failed");
    ping_pong.wait_finished();
}
