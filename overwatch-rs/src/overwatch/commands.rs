// std

// crates
use crate::overwatch::AnySettings;
use crate::services::life_cycle::LifecycleMessage;
use tokio::sync::oneshot;

// internal
use crate::services::relay::RelayResult;
use crate::services::status::StatusWatcher;
use crate::services::ServiceId;

#[derive(Debug)]
pub(crate) struct ReplyChannel<M>(pub(crate) oneshot::Sender<M>);

impl<M> From<oneshot::Sender<M>> for ReplyChannel<M> {
    fn from(sender: oneshot::Sender<M>) -> Self {
        Self(sender)
    }
}

impl<M> ReplyChannel<M> {
    pub async fn reply(self, message: M) -> Result<(), M> {
        self.0.send(message)
    }
}

/// Command for requesting communications with another service
#[derive(Debug)]
pub struct RelayCommand {
    pub(crate) service_id: ServiceId,
    pub(crate) reply_channel: ReplyChannel<RelayResult>,
}

/// Command for requesting
#[derive(Debug)]
pub struct StatusCommand {
    pub(crate) service_id: ServiceId,
    pub(crate) reply_channel: ReplyChannel<StatusWatcher>,
}

/// Command for managing [`ServiceCore`](crate::services::ServiceCore) lifecycle
#[allow(unused)]
#[derive(Debug)]
pub struct ServiceLifeCycleCommand {
    pub service_id: ServiceId,
    pub msg: LifecycleMessage,
}

/// [`Overwatch`](crate::overwatch::Overwatch) lifecycle related commands
#[derive(Debug)]
pub enum OverwatchLifeCycleCommand {
    Shutdown,
    Kill,
}

/// [`Overwatch`](crate::overwatch::Overwatch) settings update command
#[derive(Debug)]
pub struct SettingsCommand(pub(crate) AnySettings);

/// [`Overwatch`](crate::overwatch::Overwatch) tasks related commands
#[derive(Debug)]
pub enum OverwatchCommand {
    Relay(RelayCommand),
    Status(StatusCommand),
    ServiceLifeCycle(ServiceLifeCycleCommand),
    OverwatchLifeCycle(OverwatchLifeCycleCommand),
    Settings(SettingsCommand),
}
