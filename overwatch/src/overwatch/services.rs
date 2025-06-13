use async_trait::async_trait;

use crate::{
    overwatch::{handle::OverwatchHandle, Error},
    services::{lifecycle::LifecycleNotifier, relay::AnyMessage, status::StatusWatcher},
    DynError,
};

/// An Overwatch may run anything that implements this trait.
///
/// An implementor of this trait would have to handle the inner.
/// [`ServiceCore`](crate::services::ServiceCore).
#[async_trait]
pub trait Services: Sized {
    /// Inner [`ServiceCore::Settings`](crate::services::ServiceCore) grouping
    /// type.
    ///
    /// Normally this will be a settings object that groups all the inner
    /// services settings.
    type Settings;

    /// The type aggregating all different services identifiers that are part of
    /// this runtime implementation.
    ///
    /// This type is used by the services themselves to communicate with each
    /// other and to verify whether two services are part of the same runtime.
    type RuntimeServiceId;

    /// Spawn a new instance of the [`Services`] object.
    ///
    /// It returns a `(ServiceId, Runtime)` where Runtime is the
    /// [`Runtime`](tokio::runtime::Runtime) attached for each service.
    ///
    /// It also returns an instance of the implementing type.
    ///
    /// # Errors
    ///
    /// The implementer's creation error.
    fn new(
        settings: Self::Settings,
        overwatch_handle: OverwatchHandle<Self::RuntimeServiceId>,
    ) -> Result<Self, DynError>;

    /// Start a service attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn start(&mut self, service_id: &Self::RuntimeServiceId) -> Result<(), Error>;

    /// Start a list of services attached to the trait implementer.
    ///
    /// # Implementation Details
    ///
    /// The current implementation of this function (when derived via the
    /// [`#[derive_services]`](overwatch_derive::derive_services) macro)
    /// starts the services sequentially, in the order they are provided in the
    /// `service_ids` slice.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn start_sequence(&mut self, service_ids: &[Self::RuntimeServiceId])
        -> Result<(), Error>;

    /// Start all services attached to the trait implementer.
    ///
    /// # Implementation Details
    ///
    /// The current implementation of this function (when derived via the
    /// [`#[derive_services]`](overwatch_derive::derive_services) macro)
    /// starts all the services sequentially, in the order they are defined
    /// in the implementer's definition.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn start_all(&mut self) -> Result<(), Error>;

    /// Stop a service attached to the trait implementer.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn stop(&mut self, service_id: &Self::RuntimeServiceId) -> Result<(), Error>;

    /// Stop a list of services attached to the trait implementer.
    ///
    /// # Implementation Details
    ///
    /// The current implementation of this function (when derived via the
    /// [`#[derive_services]`](overwatch_derive::derive_services) macro),
    /// stops the services sequentially, in the order they are provided in the
    /// `service_ids` slice.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn stop_sequence(&mut self, service_ids: &[Self::RuntimeServiceId]) -> Result<(), Error>;

    /// Stop all services attached to the trait implementer.
    ///
    /// # Implementation Details
    ///
    /// The current implementation of this function (when derived via the
    /// [`#[derive_services]`](overwatch_derive::derive_services) macro)
    /// stops all the services sequentially, in the order they are defined
    /// in the implementer's definition.
    ///
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn stop_all(&mut self) -> Result<(), Error>;

    /// Shuts down the `Service`'s
    /// [`ServiceRunner`](crate::services::runner::ServiceRunner)s attached to
    /// the trait implementer.
    ///
    /// Depending on the implementation, this may be a no-op.
    ///
    /// This is the opposite operation of [`Self::new`]: It's _final_.
    /// `Service`s won't be able to be started again after calling it.
    ///
    /// # Implementation Details
    ///
    /// The current implementation of this function (when derived via the
    /// [`#[derive_services]`](overwatch_derive::derive_services) macro)
    /// kills the [`ServiceRunner`](crate::services::runner::ServiceRunner)s
    /// without waiting for their respective `Service`s to finish.
    /// If you want to wait for the `Service`s to finish, you should call
    /// [`Self::stop_all`] first.
    ///
    /// # Errors
    ///
    /// The generated [`Error`](enum@Error).
    async fn teardown(self) -> Result<(), Error>;

    /// Get the list of all the `RuntimeServiceId`s associated with the trait
    /// implementer.
    fn ids(&self) -> Vec<Self::RuntimeServiceId>;

    /// Request a communication relay for a service attached to the trait
    /// implementer.
    fn request_relay(&mut self, service_id: &Self::RuntimeServiceId) -> AnyMessage;

    /// Request a status watcher for a service attached to the trait
    /// implementer.
    fn request_status_watcher(&self, service_id: &Self::RuntimeServiceId) -> StatusWatcher;

    /// Update service settings for all services attached to the trait
    /// implementer.
    fn update_settings(&mut self, settings: Self::Settings);

    /// Get the [`LifecycleNotifier`] for a service attached to the trait
    /// implementer.
    fn get_service_lifecycle_notifier(
        &self,
        service_id: &Self::RuntimeServiceId,
    ) -> &LifecycleNotifier;
}
