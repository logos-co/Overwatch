use tokio::sync::oneshot;

use crate::{
    overwatch::AnySettings,
    services::{life_cycle::LifecycleMessage, relay::RelayResult, status::StatusWatcher},
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
#[derive(Debug)]
pub struct RelayCommand<AggregatedServiceId> {
    pub(crate) service_id: AggregatedServiceId,
    pub(crate) reply_channel: ReplyChannel<RelayResult>,
}

/// Command for requesting
/// [`ServiceStatus`](crate::services::status::ServiceStatus) updates
/// from another service.
#[derive(Debug)]
pub struct StatusCommand<AggregatedServiceId> {
    pub(crate) service_id: AggregatedServiceId,
    pub(crate) reply_channel: ReplyChannel<StatusWatcher>,
}

/// Command for managing [`ServiceCore`](crate::services::ServiceCore)
/// lifecycle.
#[derive(Debug)]
pub struct ServiceLifeCycleCommand<AggregatedServiceId> {
    pub service_id: AggregatedServiceId,
    pub msg: LifecycleMessage,
}

/// [`Overwatch`](crate::overwatch::Overwatch) lifecycle related commands.
#[derive(Debug)]
pub enum OverwatchLifeCycleCommand {
    Shutdown,
    Kill,
}

/// [`Overwatch`](crate::overwatch::Overwatch) settings update command.
#[derive(Debug)]
pub struct SettingsCommand(pub(crate) AnySettings);

/// [`Overwatch`](crate::overwatch::Overwatch) tasks related commands.
#[derive(Debug)]
pub enum OverwatchCommand<AggregatedServiceId> {
    Relay(RelayCommand<AggregatedServiceId>),
    Status(StatusCommand<AggregatedServiceId>),
    ServiceLifeCycle(ServiceLifeCycleCommand<AggregatedServiceId>),
    OverwatchLifeCycle(OverwatchLifeCycleCommand),
    Settings(SettingsCommand),
}
