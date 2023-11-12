// std
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
// crates
use async_trait::async_trait;
use futures::StreamExt;
use tokio::sync::watch::{channel, Receiver, Ref, Sender};
use tokio_stream::wrappers::WatchStream;
use tracing::error;
// internal

// TODO: Constrain this, probably with needed serialize/deserialize options.
/// Service state initialization traits
/// It defines what is needed for a service state to be initialized.
/// Need what set of settings information is required for it to be initialized [`ServiceState::Settings`]
/// which usually is bound to the service itself [`crate::services::ServiceData::Settings`]
pub trait ServiceState: Sized {
    /// Settings object that the state can be initialized from
    type Settings;
    /// Errors that can occur during state initialization
    type Error: std::error::Error;
    /// Initialize a stage upon the provided settings
    fn from_settings(settings: &Self::Settings) -> Result<Self, Self::Error>;
}

/// A state operator is an entity that can handle a state in a point of time
/// to perform any operation based on it.
#[async_trait]
pub trait StateOperator {
    /// The type of state that the operator can handle
    type StateInput: ServiceState;
    /// Operator initialization method. Can be implemented over some subset of settings
    fn from_settings<Settings>(settings: Settings) -> Self;
    /// Asynchronously perform an operation for a given state
    async fn run(&mut self, state: Self::StateInput);
}

/// Operator that doesn't perform any operation upon state update
#[derive(Copy)]
pub struct NoOperator<StateInput>(PhantomData<*const StateInput>);

// NoOperator does not actually hold anything and is thus Sync.
// Note that we don't use PhantomData<StateInput> as that would
// suggest we indeed hold an instance of StateInput, see
// https://doc.rust-lang.org/std/marker/struct.PhantomData.html#ownership-and-the-drop-check
unsafe impl<T> Send for NoOperator<T> {}

// auto derive introduces unnecessary Clone bound on T
impl<T> Clone for NoOperator<T> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

#[async_trait]
impl<StateInput: ServiceState> StateOperator for NoOperator<StateInput> {
    type StateInput = StateInput;

    fn from_settings<Settings>(_settings: Settings) -> Self {
        NoOperator(PhantomData)
    }

    fn run<'borrow, 'fut>(
        &'borrow mut self,
        _state: Self::StateInput,
    ) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + 'fut>>
    where
        'borrow: 'fut,
        Self: 'fut,
    {
        Box::pin(async {})
    }
}

#[derive(Debug)]
pub struct VoidError;

impl core::fmt::Display for VoidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "void error")
    }
}

impl std::error::Error for VoidError {}

/// Empty state
#[derive(Copy)]
pub struct NoState<Settings>(PhantomData<Settings>);

// auto derive introduces unnecessary Clone bound on T
impl<T> Clone for NoState<T> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<Settings> ServiceState for NoState<Settings> {
    type Settings = Settings;

    type Error = VoidError;

    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self(Default::default()))
    }
}

/// Receiver part of the state handling mechanism.
/// A state handle watches a stream of incoming states and triggers the attached operator handling
/// method over it.
pub struct StateHandle<S, Operator> {
    watcher: StateWatcher<S>,
    operator: Operator,
}

// auto derive introduces unnecessary Clone bound on T
impl<S, O> Clone for StateHandle<S, O>
where
    O: Clone,
{
    fn clone(&self) -> Self {
        Self {
            watcher: self.watcher.clone(),
            operator: self.operator.clone(),
        }
    }
}

/// Sender part of the state handling mechanism.
/// Update the current state and notifies the [`StateHandle`].
pub struct StateUpdater<S> {
    sender: Arc<Sender<S>>,
}

// auto derive introduces unnecessary Clone bound on T
impl<T> Clone for StateUpdater<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

/// Wrapper over [`tokio::sync::watch::Receiver`]
pub struct StateWatcher<S> {
    receiver: Receiver<S>,
}

// auto derive introduces unnecessary Clone bound on T
impl<T> Clone for StateWatcher<T> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}

impl<S: ServiceState> StateUpdater<S> {
    /// Send a new state and notify the [`StateWatcher`]
    pub fn update(&self, new_state: S) {
        self.sender.send(new_state).unwrap_or_else(|_e| {
            error!("Error updating state");
        });
    }
}

impl<S> StateWatcher<S>
where
    S: ServiceState + Clone,
{
    /// Get a copy of the most updated state
    pub fn state_cloned(&self) -> S {
        self.receiver.borrow().clone()
    }
}

impl<S> StateWatcher<S>
where
    S: ServiceState,
{
    /// Get a [`Ref`](tokio::sync::watch::Ref) to the last state, this blocks incoming updates until
    /// the `Ref` is dropped. Use with caution.
    pub fn state_ref(&self) -> Ref<S> {
        self.receiver.borrow()
    }
}

impl<S, O> StateHandle<S, O> {
    pub fn new(initial_state: S, operator: O) -> (Self, StateUpdater<S>) {
        let (sender, receiver) = channel(initial_state);
        let watcher = StateWatcher { receiver };
        let updater = StateUpdater {
            sender: Arc::new(sender),
        };

        (Self { watcher, operator }, updater)
    }
}

impl<S, Operator> StateHandle<S, Operator>
where
    S: ServiceState + Clone + Send + Sync + 'static,
    Operator: StateOperator<StateInput = S>,
{
    /// Wait for new state updates and run the operator handling method
    pub async fn run(self) {
        let Self {
            watcher,
            mut operator,
        } = self;
        let mut state_stream = WatchStream::new(watcher.receiver);
        while let Some(state) = state_stream.next().await {
            operator.run(state).await;
        }
    }
}

#[cfg(test)]
mod test {
    use crate::services::state::{ServiceState, StateHandle, StateOperator, StateUpdater};
    use async_trait::async_trait;
    use std::time::Duration;
    use tokio::io;
    use tokio::io::AsyncWriteExt;
    use tokio::time::sleep;

    #[derive(Clone)]
    struct UsizeCounter(usize);

    impl ServiceState for UsizeCounter {
        type Settings = ();
        type Error = std::convert::Infallible;
        fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
            Ok(Self(0))
        }
    }

    struct PanicOnGreaterThanTen;

    #[async_trait]
    impl StateOperator for PanicOnGreaterThanTen {
        type StateInput = UsizeCounter;

        fn from_settings<Settings>(_settings: Settings) -> Self {
            Self
        }

        async fn run(&mut self, state: Self::StateInput) {
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
    #[should_panic]
    async fn state_stream_collects() {
        let (handle, updater): (
            StateHandle<UsizeCounter, PanicOnGreaterThanTen>,
            StateUpdater<UsizeCounter>,
        ) = StateHandle::new(
            UsizeCounter::from_settings(&()).unwrap(),
            PanicOnGreaterThanTen::from_settings(()),
        );
        tokio::task::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            for i in 0..15 {
                updater.update(UsizeCounter(i));
                sleep(Duration::from_millis(50)).await;
            }
        });
        handle.run().await;
    }
}
