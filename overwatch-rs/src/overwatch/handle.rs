// std

// crates
use crate::overwatch::commands::{OverwatchCommand, OverwatchLifeCycleCommand, SettingsCommand};
use crate::overwatch::Services;
use crate::services::ServiceData;
use tokio::runtime::Handle;
use tokio::sync::mpsc::Sender;
use tracing::{error, info, instrument};

// internal
use crate::services::relay::Relay;

#[async_trait::async_trait]
pub trait OverwatchHandler {
    fn new(runtime_handle: Handle, sender: Sender<OverwatchCommand>) -> Self
    where
        Self: Sized;

    /// Request for a relay
    fn relay<S: ServiceData>(&self) -> Relay<S>;

    /// Send a shutdown signal to the overwatch runner
    async fn shutdown(&self);

    /// Send a kill signal to the overwatch runner
    async fn kill(&self);

    /// Send an overwatch command to the overwatch runner
    async fn send(&self, command: OverwatchCommand);

    async fn update_settings<S: Services>(&self, settings: S::Settings)
    where
        S::Settings: Send;

    fn runtime(&self) -> &Handle;
}

/// Handler object over the main Overwatch runner
/// It handles communications to the main Overwatch runner.
#[derive(Clone, Debug)]
pub struct OverwatchHandle {
    #[allow(unused)]
    runtime_handle: Handle,
    sender: Sender<OverwatchCommand>,
}

#[async_trait::async_trait]
impl OverwatchHandler for OverwatchHandle {
    fn new(runtime_handle: Handle, sender: Sender<OverwatchCommand>) -> Self {
        Self {
            runtime_handle,
            sender,
        }
    }

    /// Request for a relay
    fn relay<S: ServiceData>(&self) -> Relay<S> {
        Relay::new(self.clone())
    }

    /// Send a shutdown signal to the overwatch runner
    async fn shutdown(&self) {
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
    async fn kill(&self) {
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
    #[instrument(name = "overwatch-command-send", skip(self))]
    async fn send(&self, command: OverwatchCommand) {
        if let Err(e) = self.sender.send(command).await {
            error!(error=?e, "Error sending overwatch command");
        }
    }

    #[instrument(skip(self))]
    async fn update_settings<S: Services>(&self, settings: S::Settings)
    where
        S::Settings: Send,
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

    fn runtime(&self) -> &Handle {
        &self.runtime_handle
    }
}
