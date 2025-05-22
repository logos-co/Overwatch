pub mod fuse;
pub mod handle;
pub mod operator;
pub mod service_state;
pub mod updater;
pub mod watcher;

pub use handle::StateHandle;
pub use operator::{NoOperator, StateOperator};
pub use service_state::{NoState, ServiceState};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
pub use updater::StateUpdater;
pub use watcher::StateWatcher;

pub(crate) type Sender<State> = watch::Sender<State>;
pub(crate) type Receiver<State> = watch::Receiver<State>;
pub(crate) type Stream<State> = WatchStream<State>;
pub(crate) fn channel<State>(initial_state: State) -> (Sender<State>, Receiver<State>) {
    watch::channel(initial_state)
}
