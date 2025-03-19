use std::fmt::Debug;

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
        ServiceData,
    },
};

/// Handler object over the main [`crate::overwatch::Overwatch`] runner.
///
/// It handles communications to the main
/// [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
#[derive(Clone, Debug)]
pub struct OverwatchHandle {
    runtime_handle: Handle,
    sender: Sender<OverwatchCommand>,
}

impl OverwatchHandle {
    #[must_use]
    pub const fn new(runtime_handle: Handle, sender: Sender<OverwatchCommand>) -> Self {
        Self {
            runtime_handle,
            sender,
        }
    }

    /// Request a relay with a service
    pub async fn relay<Service>(&self) -> Result<OutboundRelay<Service::Message>, RelayError>
    where
        Service: ServiceData,
        Service::Message: 'static,
    {
        info!("Requesting relay with {}", Service::SERVICE_ID);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let Ok(()) = self
            .send(OverwatchCommand::Relay(RelayCommand {
                service_id: Service::SERVICE_ID,
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
    pub async fn status_watcher<Service: ServiceData>(&self) -> StatusWatcher {
        info!("Requesting status watcher for {}", Service::SERVICE_ID);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let Ok(()) = self
            .send(OverwatchCommand::Status(StatusCommand {
                service_id: Service::SERVICE_ID,
                reply_channel: ReplyChannel::from(sender),
            }))
            .await
        else {
            unreachable!("Service watcher should always be available");
        };
        receiver.await.unwrap_or_else(|_| {
            panic!(
                "Service {} watcher should always be available",
                Service::SERVICE_ID
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
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner)
    ///
    /// # Errors
    ///
    /// If an error occurs while trying to send the command.
    #[cfg_attr(
        feature = "instrumentation",
        instrument(name = "overwatch-command-send", skip(self))
    )]
    pub async fn send(&self, command: OverwatchCommand) -> Result<(), SendError<OverwatchCommand>> {
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

    #[must_use]
    pub const fn runtime(&self) -> &Handle {
        &self.runtime_handle
    }
}
