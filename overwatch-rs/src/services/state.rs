use std::convert::Infallible;
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

// TODO: Constrain this, probably with needed serialize/deserialize options.
/// Service state initialization traits
/// It defines what is needed for a service state to be initialized.
/// It contains the [`ServiceState::Settings`] required to initialize the service. It's usually
/// bound to the service itself [`crate::services::ServiceData::Settings`]
pub trait ServiceState: Sized {
    /// Settings object that the state can be initialized from
    type Settings;
    /// Errors that can occur during state initialization
    type Error;
    /// Initialize a state using the provided settings.
    /// This is called when [`StateOperator::try_load`] doesn't return a state.
    fn from_settings(settings: &Self::Settings) -> Result<Self, Self::Error>;
}

/// A [`StateOperator`] is an entity that performs an operation on a
/// [`ServiceData::State`](crate::services::ServiceData::State) snapshot.
/// A typical use case is to handle recovery: Saving and loading state.
#[async_trait]
pub trait StateOperator {
    /// The type of state that the operator can handle
    /// Usually the same as the [`ServiceState::Settings`]
    type StateInput;

    /// The settings to configure the operator
    /// Usually the same as the [`ServiceState::Settings`]
    type Settings;

    /// Errors that can occur during state loading
    type LoadError;

    /// State initialization method
    /// This is called to (attempt to) generate the `Service`s initial state before using the
    /// default mechanism.
    /// The reason is one of the main use cases for an operator is to handle recovery and,
    /// therefore, if the [`StateOperator`] can save a state, it should also be able to load it; so
    /// the full responsibility lies in the same entity.
    fn try_load(settings: &Self::Settings) -> Result<Option<Self::StateInput>, Self::LoadError>;

    /// Operator initialization method. Can be implemented over some subset of settings
    fn from_settings(settings: Self::Settings) -> Self;

    /// Asynchronously perform an operation for a given state snapshot
    async fn run(&mut self, state: Self::StateInput);
}

/// Operator that doesn't perform any operation upon state update
#[derive(Copy)]
pub struct NoOperator<StateInput, Settings>(PhantomData<(*const StateInput, *const Settings)>);

/// NoOperator does not actually hold anything and is thus Sync.
///
/// Note that we don't use PhantomData<StateInput> as that would suggest we indeed hold an instance
/// of [`StateOperator::StateInput`].
///
/// See: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#ownership-and-the-drop-check
unsafe impl<StateInput, Settings> Send for NoOperator<StateInput, Settings> {}

// Clone is implemented manually because auto deriving introduces an unnecessary Clone bound on T
impl<StateInput, Settings> Clone for NoOperator<StateInput, Settings> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

#[async_trait]
impl<StateInput, Settings> StateOperator for NoOperator<StateInput, Settings> {
    type StateInput = StateInput;
    type Settings = Settings;
    type LoadError = Infallible;

    fn try_load(_settings: &Self::Settings) -> Result<Option<Self::StateInput>, Self::LoadError> {
        Ok(None)
    }

    fn from_settings(_settings: Self::Settings) -> Self {
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

/// Empty state
#[derive(Copy)]
pub struct NoState<Settings>(PhantomData<Settings>);

// Clone is implemented manually because auto deriving introduces an unnecessary Clone bound on T
impl<Settings> Clone for NoState<Settings> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<Settings> ServiceState for NoState<Settings> {
    type Settings = Settings;
    type Error = crate::DynError;

    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self(PhantomData))
    }
}

/// Receiver part of the state handling mechanism.
/// A [`StateHandle`] watches a stream of incoming states and triggers the attached operator
/// handling method over it.
pub struct StateHandle<State, Operator> {
    watcher: StateWatcher<State>,
    operator: Operator,
}

// Clone is implemented manually because auto deriving introduces an unnecessary Clone bound on T
impl<State, Operator> Clone for StateHandle<State, Operator>
where
    Operator: Clone,
{
    fn clone(&self) -> Self {
        Self {
            watcher: self.watcher.clone(),
            operator: self.operator.clone(),
        }
    }
}

impl<State, Operator> StateHandle<State, Operator> {
    pub fn new(initial_state: State, operator: Operator) -> (Self, StateUpdater<State>) {
        let (sender, receiver) = channel(initial_state);
        let watcher = StateWatcher { receiver };
        let updater = StateUpdater {
            sender: Arc::new(sender),
        };

        (Self { watcher, operator }, updater)
    }
}

/// Sender part of the state handling mechanism.
/// Update the current state and notifies the [`StateHandle`].
pub struct StateUpdater<State> {
    sender: Arc<Sender<State>>,
}

// Clone is implemented manually because auto deriving introduces an unnecessary Clone bound on T
impl<State> Clone for StateUpdater<State> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<State> StateUpdater<State> {
    /// Send a new state and notify the [`StateWatcher`]
    pub fn update(&self, new_state: State) {
        self.sender.send(new_state).unwrap_or_else(|_e| {
            error!("Error updating state");
        });
    }
}

/// Wrapper over [`Receiver`]
pub struct StateWatcher<State> {
    receiver: Receiver<State>,
}

// Clone is implemented manually because auto deriving introduces an unnecessary Clone bound on T
impl<State> Clone for StateWatcher<State> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}

impl<State> StateWatcher<State>
where
    State: Clone,
{
    /// Get a copy of the most updated state
    #[must_use]
    pub fn state_cloned(&self) -> State {
        self.receiver.borrow().clone()
    }
}

impl<State> StateWatcher<State> {
    /// Get a [`Ref`] to the last state, this blocks incoming updates until the `Ref` is dropped.
    /// Use with caution.
    #[must_use]
    pub fn state_ref(&self) -> Ref<State> {
        self.receiver.borrow()
    }
}

impl<State, Operator> StateHandle<State, Operator>
where
    State: Clone + Send + Sync + 'static,
    Operator: StateOperator<StateInput = State>,
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
    use std::convert::Infallible;
    use std::time::Duration;
    use tokio::io;
    use tokio::io::AsyncWriteExt;
    use tokio::time::sleep;

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
        type StateInput = UsizeCounter;
        type Settings = ();
        type LoadError = Infallible;

        fn try_load(
            _settings: &<Self::StateInput as ServiceState>::Settings,
        ) -> Result<Option<Self::StateInput>, Self::LoadError> {
            Ok(None)
        }

        fn from_settings(_settings: <Self::StateInput as ServiceState>::Settings) -> Self {
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
    #[should_panic(expected = "assertion failed: value < 10")]
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
