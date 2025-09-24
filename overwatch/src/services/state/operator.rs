use std::{convert::Infallible, marker::PhantomData, pin::Pin};

use async_trait::async_trait;

use crate::services::state::ServiceState;

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
    /// The implementer's [`Self::LoadError`].
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

/// SAFETY: `NoOperator` does not hold anything and is thus Sync.
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
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'fut>>
    where
        'borrow: 'fut,
        Self: 'fut,
    {
        Box::pin(async {})
    }
}
