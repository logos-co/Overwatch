use std::fmt::Debug;

use tokio::{runtime::Handle, sync::mpsc::Sender};
#[cfg(feature = "instrumentation")]
use tracing::instrument;
use tracing::{error, info};

// internal
use crate::services::relay::Relay;
use crate::{
    overwatch::{
        commands::{
            OverwatchCommand, OverwatchLifeCycleCommand, ReplyChannel, SettingsCommand,
            StatusCommand,
        },
        Services,
    },
    services::{status::StatusWatcher, ServiceData},
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

    #[must_use]
    /// Request a relay
    pub fn relay<Service>(&self) -> Relay<Service>
    where
        Service: ServiceData,
        Service::Message: 'static,
    {
        Relay::new(self.clone())
    }

    /// Request a [`StatusWatcher`] for a service
    ///
    /// # Panics
    /// If the service watcher is not available.
    pub async fn status_watcher<Service: ServiceData>(&self) -> StatusWatcher {
        info!("Requesting status watcher for {}", Service::SERVICE_ID);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let watcher_request = self
            .sender
            .send(OverwatchCommand::Status(StatusCommand {
                service_id: Service::SERVICE_ID,
                reply_channel: ReplyChannel::from(sender),
            }))
            .await;
        match watcher_request {
            Ok(()) => receiver.await.unwrap_or_else(|_| {
                panic!(
                    "Service {} watcher should always be available",
                    Service::SERVICE_ID
                )
            }),
            Err(_) => {
                unreachable!("Service watcher should always be available");
            }
        }
    }

    /// Send a shutdown signal to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner)
    pub async fn shutdown(&self) {
        info!("Shutting down Overwatch");
        if let Err(e) = self
            .sender
            .send(OverwatchCommand::OverwatchLifeCycle(
                OverwatchLifeCycleCommand::Shutdown,
            ))
            .await
        {
            dbg!(e);
        }
    }

    /// Send a kill signal to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner)
    pub async fn kill(&self) {
        info!("Killing Overwatch");
        if let Err(e) = self
            .sender
            .send(OverwatchCommand::OverwatchLifeCycle(
                OverwatchLifeCycleCommand::Kill,
            ))
            .await
        {
            dbg!(e);
        }
    }

    /// Send an overwatch command to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner)
    #[cfg_attr(
        feature = "instrumentation",
        instrument(name = "overwatch-command-send", skip(self))
    )]
    pub async fn send(&self, command: OverwatchCommand) {
        if let Err(e) = self.sender.send(command).await {
            error!(error=?e, "Error sending overwatch command");
        }
    }

    #[cfg_attr(feature = "instrumentation", instrument(skip(self)))]
    pub async fn update_settings<S: Services>(&self, settings: S::Settings)
    where
        S::Settings: Send + Debug + 'static,
    {
        if let Err(e) = self
            .sender
            .send(OverwatchCommand::Settings(SettingsCommand(Box::new(
                settings,
            ))))
            .await
        {
            error!(error=?e, "Error updating settings");
        }
    }

    #[must_use]
    pub const fn runtime(&self) -> &Handle {
        &self.runtime_handle
    }
}
