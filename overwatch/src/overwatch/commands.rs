use tokio::sync::oneshot;

use crate::{
    overwatch::AnySettings,
    services::{life_cycle::LifecycleMessage, relay::AnyMessage, status::StatusWatcher},
};

#[derive(Debug)]
pub(crate) struct ReplyChannel<Message>(pub(crate) oneshot::Sender<Message>);

impl<Message> From<oneshot::Sender<Message>> for ReplyChannel<Message> {
    fn from(sender: oneshot::Sender<Message>) -> Self {
        Self(sender)
    }
}

impl<Message> ReplyChannel<Message> {
    pub fn reply(self, message: Message) -> Result<(), Message> {
        self.0.send(message)
    }
}

/// Command for requesting communications with another service.
///
/// Commands can only be sent to other services that are aggregated under the
/// same `RuntimeServiceId`, i.e., they are part of the same overwatch runtime.
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

/// Command for managing [`ServiceCore`](crate::services::ServiceCore)
/// lifecycle.
#[derive(Debug)]
pub struct ServiceLifeCycleCommand<RuntimeServiceId> {
    pub service_id: RuntimeServiceId,
    pub msg: LifecycleMessage,
}

/// Command for managing [`Overwatch`](crate::overwatch::Overwatch)
/// lifecycle.
// TODO: Due to the variant's names a broader `OverwatchCommand` might be more suitable.
#[derive(Debug)]
pub enum OverwatchLifeCycleCommand {
    /// Starts all `Service`s associated to an
    /// [`Overwatch`](crate::overwatch::Overwatch) instance.
    StartAllServices,
    /// Stops all `Service`s associated to an
    /// [`Overwatch`](crate::overwatch::Overwatch) instance.
    StopAllServices,
    /// Shuts down [`Overwatch`](crate::overwatch::Overwatch), sending the
    /// `finish_runner_signal`
    /// to [`Overwatch`](crate::overwatch::Overwatch). It's the signal which
    /// [`Overwatch::wait_finished`](crate::overwatch::Overwatch::wait_finished)
    /// awaits.
    ///
    /// This message is final: It stops all `Service`s (and their respective
    /// [`ServiceRunner`](crate::services::runner::ServiceRunner)s) so
    /// `Service`s can't be started again.
    Shutdown,
}

/// [`Overwatch`](crate::overwatch::Overwatch) settings update command.
#[derive(Debug)]
pub struct SettingsCommand(pub(crate) AnySettings);

/// [`Overwatch`](crate::overwatch::Overwatch) tasks related commands.
#[derive(Debug)]
pub enum OverwatchCommand<RuntimeServiceId> {
    Relay(RelayCommand<RuntimeServiceId>),
    Status(StatusCommand<RuntimeServiceId>),
    ServiceLifeCycle(ServiceLifeCycleCommand<RuntimeServiceId>),
    OverwatchLifeCycle(OverwatchLifeCycleCommand),
    Settings(SettingsCommand),
}
