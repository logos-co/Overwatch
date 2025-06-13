use tokio::sync::oneshot;

use crate::{
    overwatch::AnySettings,
    services::{relay::AnyMessage, status::StatusWatcher},
    utils::finished_signal,
};

#[derive(Debug)]
pub struct ReplyChannel<Message>(pub(crate) oneshot::Sender<Message>);

impl<Message> From<oneshot::Sender<Message>> for ReplyChannel<Message> {
    fn from(sender: oneshot::Sender<Message>) -> Self {
        Self(sender)
    }
}

impl<Message> ReplyChannel<Message> {
    /// Sends a reply message back to the requester.
    ///
    /// # Errors
    pub fn reply(self, message: Message) -> Result<(), Message> {
        self.0.send(message)
    }
}

/// Command for requesting communications with another service.
///
/// Commands can only be sent to other services that are aggregated under the
/// same `RuntimeServiceId`, i.e. they are part of the same
/// [`Overwatch`](overwatch::Overwatch) runtime.
#[derive(Debug)]
pub struct RelayCommand<RuntimeServiceId> {
    pub(crate) service_id: RuntimeServiceId,
    pub(crate) reply_channel: ReplyChannel<AnyMessage>,
}

/// Command for requesting
/// [`ServiceStatus`](crate::services::status::ServiceStatus) updates
/// from another service.
#[derive(Debug)]
pub struct StatusCommand<RuntimeServiceId> {
    pub(crate) service_id: RuntimeServiceId,
    pub(crate) reply_channel: ReplyChannel<StatusWatcher>,
}

#[derive(Debug)]
pub struct ServiceSingleCommand<RuntimeServiceId> {
    pub service_id: RuntimeServiceId,
    pub sender: finished_signal::Sender,
}

#[derive(Debug)]
pub struct ServiceSequenceCommand<RuntimeServiceId> {
    pub service_ids: Vec<RuntimeServiceId>,
    pub sender: finished_signal::Sender,
}

#[derive(Debug)]
pub struct ServiceAllCommand {
    pub sender: finished_signal::Sender,
}

/// Commands for managing [`Service`](crate::services::Service)s lifecycle.
#[derive(Debug)]
pub enum ServiceLifecycleCommand<RuntimeServiceId> {
    /// Starts a `Service` associated with an
    /// [`Overwatch`](overwatch::Overwatch) instance.
    StartService(ServiceSingleCommand<RuntimeServiceId>),
    /// Starts a sequence of `Service`s associated with an
    /// [`Overwatch`](overwatch::Overwatch) instance.
    StartServiceSequence(ServiceSequenceCommand<RuntimeServiceId>),
    /// Starts all `Service`s associated with an
    /// [`Overwatch`](overwatch::Overwatch) instance.
    StartAllServices(ServiceAllCommand),
    /// Stops a `Service` associated with an
    /// [`Overwatch`](overwatch::Overwatch) instance.
    StopService(ServiceSingleCommand<RuntimeServiceId>),
    /// Stops a sequence of `Service`s associated with an
    /// [`Overwatch`](overwatch::Overwatch) instance.
    StopServiceSequence(ServiceSequenceCommand<RuntimeServiceId>),
    /// Stops all `Service`s associated with an
    /// [`Overwatch`](overwatch::Overwatch) instance.
    StopAllServices(ServiceAllCommand),
}

/// Command for everything [`Overwatch`](overwatch::Overwatch)-level operations.
#[derive(Debug)]
pub enum OverwatchManagementCommand<RuntimeServiceId> {
    /// Retrieves the list of all the `Service`s' `RuntimeServiceId`s
    RetrieveServiceIds(ReplyChannel<Vec<RuntimeServiceId>>),
    /// Shuts down [`Overwatch`](overwatch::Overwatch), sending the
    /// `finish_runner_signal`
    /// to [`Overwatch`](overwatch::Overwatch). It's the signal which
    /// [`Overwatch::wait_finished`](overwatch::Overwatch::wait_finished)
    /// awaits.
    ///
    /// This message is final: It stops all `Service`s (and their respective
    /// [`ServiceRunner`](crate::services::runner::ServiceRunner)s) so
    /// `Service`s can't be started again.
    Shutdown(finished_signal::Sender),
}

/// [`Overwatch`](overwatch::Overwatch) settings update command.
#[derive(Debug)]
pub struct SettingsCommand(pub(crate) AnySettings);

/// [`Overwatch`](overwatch::Overwatch) tasks related commands.
#[derive(Debug)]
pub enum OverwatchCommand<RuntimeServiceId> {
    Relay(RelayCommand<RuntimeServiceId>),
    Status(StatusCommand<RuntimeServiceId>),
    ServiceLifecycle(ServiceLifecycleCommand<RuntimeServiceId>),
    OverwatchManagement(OverwatchManagementCommand<RuntimeServiceId>),
    Settings(SettingsCommand),
}
