use std::marker::PhantomData;

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
    /// The generated [`Error`](Self::Error).
    fn from_settings(settings: &Self::Settings) -> Result<Self, Self::Error>;
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
