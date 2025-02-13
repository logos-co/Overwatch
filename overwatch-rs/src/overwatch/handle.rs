// std

use std::fmt::Debug;

// crates
use crate::overwatch::commands::{
    OverwatchCommand, OverwatchLifeCycleCommand, ReplyChannel, SettingsCommand, StatusCommand,
};
use crate::overwatch::Services;
use crate::services::ServiceData;
use tokio::runtime::Handle;
use tokio::sync::mpsc::Sender;
#[cfg(feature = "instrumentation")]
use tracing::instrument;
use tracing::{error, info};

// internal
use crate::services::relay::Relay;
use crate::services::status::StatusWatcher;

/// Handler object over the main Overwatch runner
/// It handles communications to the main Overwatch runner.
#[derive(Clone, Debug)]
pub struct OverwatchHandle {
    runtime_handle: Handle,
    sender: Sender<OverwatchCommand>,
}

impl OverwatchHandle {
    #[must_use]
    pub fn new(runtime_handle: Handle, sender: Sender<OverwatchCommand>) -> Self {
        Self {
            runtime_handle,
            sender,
        }
    }

    #[must_use]
    /// Request for a relay
    pub fn relay<Service>(&self) -> Relay<Service>
    where
        Service: ServiceData,
        Service::Message: 'static,
    {
        Relay::new(self.clone())
    }

    // Request a status watcher for a service
    pub async fn status_watcher<S: ServiceData>(&self) -> StatusWatcher {
        info!("Requesting status watcher for {}", S::SERVICE_ID);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let watcher_request = self
            .sender
            .send(OverwatchCommand::Status(StatusCommand {
                service_id: S::SERVICE_ID,
                reply_channel: ReplyChannel::from(sender),
            }))
            .await;
        match watcher_request {
            Ok(_) => receiver.await.unwrap_or_else(|_| {
                panic!(
                    "Service {} watcher should always be available",
                    S::SERVICE_ID
                )
            }),
            Err(_) => {
                unreachable!("Service watcher should always be available");
            }
        }
    }

    /// Send a shutdown signal to the overwatch runner
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

    /// Send a kill signal to the overwatch runner
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

    /// Send an overwatch command to the overwatch runner
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
            error!(error=?e, "Error updating settings")
        }
    }

    pub fn runtime(&self) -> &Handle {
        &self.runtime_handle
    }
}
