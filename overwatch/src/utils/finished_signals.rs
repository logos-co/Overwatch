use tokio::sync::oneshot;

pub type Signal = ();
pub type Sender = oneshot::Sender<Signal>;
pub type Receiver = oneshot::Receiver<Signal>;
pub type Channel = (Sender, Receiver);

#[must_use]
pub fn channel() -> Channel {
    oneshot::channel()
}
