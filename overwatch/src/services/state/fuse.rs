use tokio::sync::broadcast;

const CAPACITY: usize = 1;

pub type Signal = ();
pub type Sender = broadcast::Sender<Signal>;
pub type Receiver = broadcast::Receiver<Signal>;
#[must_use]
pub fn channel() -> (Sender, Receiver) {
    broadcast::channel(CAPACITY)
}
