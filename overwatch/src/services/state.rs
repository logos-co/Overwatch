use std::{convert::Infallible, marker::PhantomData, pin::Pin, sync::Arc};

use async_trait::async_trait;
use futures::FutureExt;
use tokio::sync::watch::{channel, Receiver, Sender};
use tokio_stream::{wrappers::WatchStream, StreamExt};
use tracing::error;

/// Service state initialization traits.
///
/// It defines what is required to initialize the state of a `Service`.
///
/// It contains the [`ServiceState::Settings`] required to initialize the
/// service. It's usually bound to the service itself
/// [`crate::services::ServiceData::Settings`].
// TODO: Constrain this, probably with needed serialize/deserialize options.
pub trait ServiceState: Sized {
    /// Settings object that the state can be initialized from
    ///
    /// In the standard use case -
    /// [`ServiceData::State`](crate::ServiceData::State) - it needs to
    /// match [`ServiceData::Settings`](crate::ServiceData::Settings).
    type Settings;

    /// Errors that can occur during state initialization
    type Error;

    /// Initialize a state using the provided settings.
    ///
    /// This is called when [`StateOperator::try_load`] doesn't return a state.
    ///
    /// # Errors
    ///
    /// The generated [`Error`].
    fn from_settings(settings: &Self::Settings) -> Result<Self, Self::Error>;
}

/// Performs an operation on a
/// [`ServiceData::State`](crate::services::ServiceData::State) snapshot.
///
/// A typical use case is to handle recovery: Saving and loading state.
#[async_trait]
pub trait StateOperator {
    /// The type of state that the operator can handle.
    ///
    /// In the standard use case -
    /// [`ServiceData::StateOperator`](crate::ServiceData::StateOperator) - it
    /// needs to match [`ServiceData::State`](crate::ServiceData::State).
    type State: ServiceState;

    /// Errors that can occur during state loading.
    type LoadError;

    /// State initialization method.
    ///
    /// This is called to (attempt to) generate the `Service`s initial state
    /// before using the default mechanism.
    ///
    /// The reason is one of the main use cases for an operator is to handle
    /// recovery and, therefore, if the [`StateOperator`] can save a state,
    /// it should also be able to load it; so the full responsibility lies
    /// in the same entity.
    ///
    /// # Errors
    ///
    /// The implementer's [`LoadError`].
    fn try_load(
        settings: &<Self::State as ServiceState>::Settings,
    ) -> Result<Option<Self::State>, Self::LoadError>;

    /// Operator initialization method. Can be implemented over some subset of
    /// settings.
    fn from_settings(settings: &<Self::State as ServiceState>::Settings) -> Self;

    /// Asynchronously perform an operation for a given state snapshot.
    async fn run(&mut self, state: Self::State);
}

/// Operator that doesn't perform any operation upon state update.
#[derive(Copy)]
pub struct NoOperator<StateInput>(PhantomData<*const StateInput>);

/// `NoOperator` does not hold anything and is thus Sync.
///
/// Note that we don't use `PhantomData<StateInput>` as that would suggest we
/// indeed hold an instance of [`StateOperator::State`].
///
/// [Ownership and the drop check](https://doc.rust-lang.org/std/marker/struct.PhantomData.html#ownership-and-the-drop-check)
unsafe impl<StateInput> Send for NoOperator<StateInput> {}

// Clone is implemented manually because auto deriving introduces an unnecessary
// Clone bound on T.
impl<StateInput> Clone for NoOperator<StateInput> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

#[async_trait]
impl<StateInput: ServiceState> StateOperator for NoOperator<StateInput> {
    type State = StateInput;
    type LoadError = Infallible;

    fn try_load(
        _settings: &<Self::State as ServiceState>::Settings,
    ) -> Result<Option<Self::State>, Self::LoadError> {
        Ok(None)
    }

    fn from_settings(_settings: &<Self::State as ServiceState>::Settings) -> Self {
        Self(PhantomData)
    }

    fn run<'borrow, 'fut>(
        &'borrow mut self,
        _state: Self::State,
    ) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + 'fut>>
    where
        'borrow: 'fut,
        Self: 'fut,
    {
        Box::pin(async {})
    }
}

/// Empty state.
#[derive(Copy)]
pub struct NoState<Settings>(PhantomData<Settings>);

// Clone is implemented manually because auto deriving introduces an unnecessary
// Clone bound on T.
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

pub(crate) mod fuse {
    use tokio::sync::broadcast;
    const CAPACITY: usize = 1;

    pub type Signal = ();
    pub type Sender = broadcast::Sender<Signal>;
    pub type Receiver = broadcast::Receiver<Signal>;
    pub fn channel() -> (Sender, Receiver) {
        broadcast::channel(CAPACITY)
    }
}

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
    pub fn new(
        operator: Operator,
        initial_state: Option<State>,
        operator_fuse_receiver: fuse::Receiver,
    ) -> (Self, StateUpdater<State>) {
        let (sender, receiver) = channel(initial_state);
        let watcher = StateWatcher { receiver };
        let updater = StateUpdater {
            sender: Arc::new(sender),
        };

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
            watcher,
            mut operator,
            mut operator_fuse_receiver,
        } = self;

        let mut state_stream = WatchStream::new(watcher.receiver);
        loop {
            tokio::select! {
                 _ = operator_fuse_receiver.recv() => {
                     dbg!("StateHandle's Operator loop received a fuse signal.");
                     break;
                 }
                Some(state) = state_stream.next() => {
                    dbg!("StateHandle's Stream received a state. Forwarding to Operator.");
                    Self::process_state(&mut operator, state).await;
                }
            }
        }

        dbg!("Attempting to fetch the last state from StateHandle's Stream.");
        if let Some(last_state) = state_stream.next().now_or_never().flatten() {
            dbg!("StateHandle's Stream received the last state. Forwarding to Operator.");
            Self::process_state(&mut operator, last_state).await;
        }
        dbg!("StateHandle's Operator loop finished.");
    }

    async fn process_state(operator: &mut Operator, state: Option<State>) {
        if let Some(state) = state {
            operator.run(state).await;
        } else {
            dbg!("StateHandle's Stream received None. Not forwarding to StateOperator.");
        }
    }
}

/// Sender part of the state handling mechanism.
///
/// Update the current state and notifies the [`StateHandle`].
pub struct StateUpdater<State> {
    sender: Arc<Sender<Option<State>>>,
}

// Clone is implemented manually because auto deriving introduces an unnecessary
// Clone bound on T.
impl<State> Clone for StateUpdater<State> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<State> StateUpdater<State> {
    /// Send a new state and notify the [`StateWatcher`].
    ///
    /// `None` values won't be forwarded to the [`StateOperator`].
    pub fn update(&self, new_state: Option<State>) {
        self.sender.send(new_state).unwrap_or_else(|error| {
            error!("Error updating State: {error}");
        });
    }
}

/// Receiver part of the state handling mechanism.
pub struct StateWatcher<State> {
    receiver: Receiver<State>,
}

// Clone is implemented manually because auto deriving introduces an unnecessary
// Clone bound on T.
impl<State> Clone for StateWatcher<State> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}

impl<State> StateWatcher<State> {
    /// Get the internal [`Receiver`].
    #[must_use]
    pub const fn receiver(&self) -> &Receiver<State> {
        &self.receiver
    }
}

#[cfg(test)]
mod test {
    use std::{convert::Infallible, time::Duration};

    use async_trait::async_trait;
    use tokio::{io, io::AsyncWriteExt, time::sleep};

    use crate::services::state::{fuse, ServiceState, StateHandle, StateOperator, StateUpdater};

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
        let (handle, updater): (
            StateHandle<UsizeCounter, PanicOnGreaterThanTen>,
            StateUpdater<UsizeCounter>,
        ) = StateHandle::new(
            PanicOnGreaterThanTen::from_settings(&()),
            Some(initial_state),
            operator_fuse_receiver,
        );

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
