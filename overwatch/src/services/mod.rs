/*!
# Services
This is a brief overview of the main entities of the Services module:
- [`ServiceRunner`](runner::ServiceRunner): Oneshot runner of a `Service`.
  When booted, it starts listening for [`LifecycleMessage`](life_cycle::LifecycleMessage)s for that
  `Service` and acts upon them.
  Currently: `Start` and `Stop` the `Service`.
- [`ServiceHandle`](handle::ServiceHandle): Contains the components an external source might need
  to communicate with a `Service`.
  It includes everything from mechanisms to send `Message`s to `Service`s to reading and updating
  their `Settings`.
- [`ServiceResources`](resources::ServiceResources): Contains the components a `Service` and
  [`ServiceRunner`](runner::ServiceRunner) might need to interact with itself and
  [`Overwatch`](super::overwatch::Overwatch).
  It includes everything from mechanisms to send
  [`OverwatchCommand`](super::overwatch::commands::OverwatchCommand)s to updating their own `State`.
  The ownership of [`ServiceResources`](resources::ServiceResources) belongs to
  [`ServiceRunner`](runner::ServiceRunner).
  - [`ServiceResourcesHandle`]: A clone from [`ServiceResources`](resources::ServiceResources)
    that includes the [`InboundRelay`](relay::InboundRelay) for the `Service`.
    Whenever a `Service` is started, a new clone is made.
 */

pub mod handle;
pub mod life_cycle;
pub mod relay;
pub mod resources;
pub mod runner;
pub mod settings;
pub mod state;
pub mod status;

use async_trait::async_trait;

use crate::services::resources::ServiceResourcesHandle;

/// The core data a service needs to handle.
/// Holds the necessary information of a service.
pub trait ServiceData {
    /// Service relay buffer size
    const SERVICE_RELAY_BUFFER_SIZE: usize = 16;
    /// Service settings object
    type Settings;
    /// Service state object
    type State;
    /// State operator
    type StateOperator;
    /// Service messages that the service itself understands and can react to
    type Message;
}

/// Trait implemented for services that are included in a specific Overwatch
/// handle by the aggregated runtime service ID.
// This trait is implemented by the runtime macro and must be required by
// services to be able to communicate with each other.
// It guarantees that services implementing this for the same RuntimeServiceId
// are part of the same runtime.
pub trait AsServiceId<T> {
    const SERVICE_ID: Self;
}

/// Main trait for Services initialization and main loop hook.
///
/// # Note
///
/// The 'Drop' trait handles the `On Stop` behaviour.
#[async_trait]
pub trait ServiceCore<RuntimeServiceId>: Sized + ServiceData {
    /// Initialize the service with the given handle and initial state.
    ///
    /// # Errors
    ///
    /// The initialization creation error.
    fn init(
        service_resources_handle: ServiceResourcesHandle<
            Self::Message,
            Self::Settings,
            Self::State,
            RuntimeServiceId,
        >,
        initial_state: Self::State,
    ) -> Result<Self, super::DynError>;

    /// Main loop
    async fn run(mut self) -> Result<(), super::DynError>;
}
