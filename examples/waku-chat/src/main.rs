// public chat service
// messages are disseminated through waku,
// no consensus, no blocks
mod network;
// TODO: different chat rooms with different contentTopicId
mod chat;

use chat::*;
use clap::Parser;
use network::*;
use overwatch::{overwatch::*, services::handle::ServiceHandle};
use overwatch_derive::*;
use tracing_subscriber;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Multiaddrs of other nodes participating in the protocol
    #[arg(short, long)]
    peers: Vec<String>,

    /// Listening port
    port: u16,
}

#[derive(Services)]
struct Services {
    chat: ServiceHandle<ChatService>,
    network: ServiceHandle<NetworkService<network::waku::Waku>>,
}

fn main() {
    tracing_subscriber::fmt::init();
    let Args { peers, port } = Args::parse();
    let app = OverwatchRunner::<Services>::run(
        ServicesServiceSettings {
            chat: rand::random(),
            network: NetworkConfig { peers, port },
        },
        None,
    );
    app.wait_finished();
}
