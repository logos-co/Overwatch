use std::sync::Arc;

use futures::FutureExt;
use tokio_stream::StreamExt;
use tracing::debug;

use crate::services::state::{channel, fuse, StateOperator, StateUpdater, StateWatcher, Stream};

/// Receiver part of the state handling mechanism.
///
/// A [`StateHandle`] watches a stream of incoming states and triggers the
/// attached operator handling method over it.
pub struct StateHandle<State, Operator> {
    watcher: StateWatcher<Option<State>>,
    operator: Operator,
    operator_fuse_receiver: fuse::Receiver,
}

// Clone must be used carefully. It's very likely `Operator` will be a
// `StateOperator`, which are likely behaving in as if they were singletons.
// Clone is implemented manually because auto deriving introduces an unnecessary
// Clone bound on T.
impl<State, Operator> Clone for StateHandle<State, Operator>
where
    State: Clone,
    Operator: Clone,
{
    fn clone(&self) -> Self {
        Self {
            watcher: self.watcher.clone(),
            operator: self.operator.clone(),
            operator_fuse_receiver: self.operator_fuse_receiver.resubscribe(),
        }
    }
}

impl<State, Operator> StateHandle<State, Operator> {
    pub const fn watcher(&self) -> &StateWatcher<Option<State>> {
        &self.watcher
    }

    pub const fn operator(&self) -> &Operator {
        &self.operator
    }
}

impl<State, Operator> StateHandle<State, Operator>
where
    State: Clone,
{
    /// Creates a new [`StateHandle`] and a [`StateUpdater`] to communicate with
    /// it.
    pub fn new(
        operator: Operator,
        initial_state: Option<State>,
        operator_fuse_receiver: fuse::Receiver,
    ) -> (Self, StateUpdater<Option<State>>) {
        let (sender, receiver) = channel(initial_state);
        let watcher = StateWatcher::new(receiver);
        let updater = StateUpdater::new(Arc::new(sender));

        (
            Self {
                watcher,
                operator,
                operator_fuse_receiver,
            },
            updater,
        )
    }
}

impl<State, Operator> StateHandle<State, Operator>
where
    State: Clone + Send + Sync + 'static,
    Operator: StateOperator<State = State>,
{
    /// Wait for new state updates and run the operator handling method.
    pub async fn run(self) {
        let Self {
            watcher: StateWatcher { receiver },
            mut operator,
            mut operator_fuse_receiver,
        } = self;

        let mut state_stream = Stream::new(receiver);
        loop {
            tokio::select! {
                 _ = operator_fuse_receiver.recv() => {
                     debug!("StateHandle's Operator loop received a fuse signal.");
                     break;
                 }
                Some(state) = state_stream.next() => {
                    debug!("StateHandle's Stream received a state. Forwarding to Operator.");
                    Self::process_state(&mut operator, state).await;
                }
            }
        }

        debug!("Attempting to fetch the last state from StateHandle's Stream.");
        if let Some(last_state) = state_stream.next().now_or_never().flatten() {
            debug!("StateHandle's Stream received the last state. Forwarding to Operator.");
            Self::process_state(&mut operator, last_state).await;
        }
        debug!("StateHandle's Operator loop finished.");
    }

    async fn process_state(operator: &mut Operator, state: Option<State>) {
        if let Some(state) = state {
            operator.run(state).await;
        } else {
            debug!("StateHandle's Stream received None. Not forwarding to StateOperator.");
        }
    }
}

#[cfg(test)]
mod test {
    use std::{convert::Infallible, time::Duration};

    use async_trait::async_trait;
    use tokio::{io, io::AsyncWriteExt, time::sleep};

    use crate::services::state::{
        handle::fuse, ServiceState, StateHandle, StateOperator, StateUpdater,
    };

    #[derive(Clone)]
    struct UsizeCounter(usize);

    impl ServiceState for UsizeCounter {
        type Settings = ();
        type Error = crate::DynError;
        fn from_settings(_settings: &Self::Settings) -> Result<Self, crate::DynError> {
            Ok(Self(0))
        }
    }

    struct PanicOnGreaterThanTen;

    #[async_trait]
    impl StateOperator for PanicOnGreaterThanTen {
        type State = UsizeCounter;
        type LoadError = Infallible;

        fn try_load(
            _settings: &<Self::State as ServiceState>::Settings,
        ) -> Result<Option<Self::State>, Self::LoadError> {
            Ok(None)
        }

        fn from_settings(_settings: &<Self::State as ServiceState>::Settings) -> Self {
            Self
        }

        async fn run(&mut self, state: Self::State) {
            let mut stdout = io::stdout();
            let UsizeCounter(value) = state;
            stdout
                .write_all(format!("{value}\n").as_bytes())
                .await
                .expect("stop Output wrote");
            assert!(value < 10);
        }
    }

    #[tokio::test]
    #[should_panic(expected = "assertion failed: value < 10")]
    async fn state_stream_collects() {
        let (_operator_fuse_sender, operator_fuse_receiver) = fuse::channel();
        let initial_state = UsizeCounter::from_settings(&()).unwrap();
        let settings = PanicOnGreaterThanTen::from_settings(&());
        let (handle, updater): (
            StateHandle<UsizeCounter, PanicOnGreaterThanTen>,
            StateUpdater<Option<UsizeCounter>>,
        ) = StateHandle::new(settings, Some(initial_state), operator_fuse_receiver);

        tokio::task::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            for i in 0..15 {
                updater.update(Some(UsizeCounter(i)));
                sleep(Duration::from_millis(50)).await;
            }
        });
        handle.run().await;
    }
}
