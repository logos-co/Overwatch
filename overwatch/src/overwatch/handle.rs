use std::fmt::{Debug, Display};

use tokio::{
    runtime::Handle,
    sync::mpsc::{error::SendError, Sender},
};
#[cfg(feature = "instrumentation")]
use tracing::instrument;
use tracing::{error, info};

use crate::{
    overwatch::{
        commands::{
            OverwatchCommand, OverwatchLifeCycleCommand, RelayCommand, ReplyChannel,
            SettingsCommand, StatusCommand,
        },
        Services,
    },
    services::{
        relay::{OutboundRelay, RelayError},
        status::StatusWatcher,
        AsServiceId, ServiceData,
    },
};

/// Handler object over the main [`crate::overwatch::Overwatch`] runner.
///
/// It handles communications to the main
/// [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
#[derive(Clone, Debug)]
pub struct OverwatchHandle<RuntimeServiceId> {
    runtime_handle: Handle,
    sender: Sender<OverwatchCommand<RuntimeServiceId>>,
}

impl<RuntimeServiceId> OverwatchHandle<RuntimeServiceId> {
    #[must_use]
    pub const fn new(
        runtime_handle: Handle,
        sender: Sender<OverwatchCommand<RuntimeServiceId>>,
    ) -> Self {
        Self {
            runtime_handle,
            sender,
        }
    }

    #[must_use]
    pub const fn runtime(&self) -> &Handle {
        &self.runtime_handle
    }
}

impl<RuntimeServiceId> OverwatchHandle<RuntimeServiceId>
where
    RuntimeServiceId: Debug + Sync + Display,
{
    /// Request a relay with a service
    pub async fn relay<Service>(&self) -> Result<OutboundRelay<Service::Message>, RelayError>
    where
        Service: ServiceData,
        Service::Message: 'static,
        RuntimeServiceId: AsServiceId<Service>,
    {
        info!("Requesting relay with {}", RuntimeServiceId::SERVICE_ID);
        let (sender, receiver) = tokio::sync::oneshot::channel();

        let Ok(()) = self
            .send(OverwatchCommand::Relay(RelayCommand {
                service_id: RuntimeServiceId::SERVICE_ID,
                reply_channel: ReplyChannel::from(sender),
            }))
            .await
        else {
            unreachable!("Service relay should always be available");
        };
        let message = receiver
            .await
            .map_err(|e| RelayError::Receiver(Box::new(e)))??;
        let Ok(downcasted_message) = message.downcast::<OutboundRelay<Service::Message>>() else {
            unreachable!("Statically should always be of the correct type");
        };
        Ok(*downcasted_message)
    }

    /// Request a [`StatusWatcher`] for a service
    ///
    /// # Panics
    /// If the service watcher is not available.
    pub async fn status_watcher<Service>(&self) -> StatusWatcher
    where
        RuntimeServiceId: AsServiceId<Service>,
    {
        info!(
            "Requesting status watcher for {}",
            RuntimeServiceId::SERVICE_ID
        );
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let Ok(()) = self
            .send(OverwatchCommand::Status(StatusCommand {
                service_id: RuntimeServiceId::SERVICE_ID,
                reply_channel: ReplyChannel::from(sender),
            }))
            .await
        else {
            unreachable!("Service watcher should always be available");
        };
        receiver.await.unwrap_or_else(|_| {
            panic!(
                "Service {} watcher should always be available",
                RuntimeServiceId::SERVICE_ID
            )
        })
    }

    /// Send a shutdown signal to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner)
    pub async fn shutdown(&self) {
        info!("Shutting down Overwatch");
        let _: Result<(), _> = self
            .send(OverwatchCommand::OverwatchLifeCycle(
                OverwatchLifeCycleCommand::Shutdown,
            ))
            .await
            .map_err(|e| dbg!(e));
    }

    /// Send a kill signal to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner)
    pub async fn kill(&self) {
        info!("Killing Overwatch");
        let _: Result<(), _> = self
            .send(OverwatchCommand::OverwatchLifeCycle(
                OverwatchLifeCycleCommand::Kill,
            ))
            .await
            .map_err(|e| dbg!(e));
    }

    /// Send an overwatch command to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
    ///
    /// # Errors
    ///
    /// If the received side of the channel is closed and the message cannot be
    /// sent.
    #[cfg_attr(
        feature = "instrumentation",
        instrument(name = "overwatch-command-send", skip(self))
    )]
    pub async fn send(
        &self,
        command: OverwatchCommand<RuntimeServiceId>,
    ) -> Result<(), SendError<OverwatchCommand<RuntimeServiceId>>> {
        self.sender.send(command).await.map_err(|e| {
            error!(error=?e, "Error sending overwatch command");
            e
        })
    }

    #[cfg_attr(feature = "instrumentation", instrument(skip(self)))]
    pub async fn update_settings<S: Services>(&self, settings: S::Settings)
    where
        S::Settings: Send + Debug + 'static,
    {
        let _: Result<(), _> = self
            .send(OverwatchCommand::Settings(SettingsCommand(Box::new(
                settings,
            ))))
            .await
            .map_err(|e| error!(error=?e, "Error updating settings"));
    }
}
