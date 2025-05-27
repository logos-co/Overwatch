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
            ServiceLifeCycleCommand, SettingsCommand, StatusCommand,
        },
        Services,
    },
    services::{
        lifecycle::LifecycleMessage,
        relay::{OutboundRelay, RelayError, ServiceError},
        status::StatusWatcher,
        AsServiceId, ServiceData,
    },
};

/// Handler object over the main [`crate::overwatch::Overwatch`] runner.
///
/// It handles communications to the main
/// [`OverwatchRunner`](crate::overwatch::OverwatchRunner) for services that are
/// part of the same runtime, i.e., aggregated under the same
/// `RuntimeServiceId`.
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
    /// Request a relay with a service.
    ///
    /// # Errors
    ///
    /// If the relay cannot be created, or if the service is not available.
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
            .map_err(|e| RelayError::Receiver(Box::new(e)))?;
        let Ok(downcasted_message) = message.downcast::<OutboundRelay<Service::Message>>() else {
            unreachable!("Statically should always be of the correct type");
        };
        Ok(*downcasted_message)
    }

    /// Request a [`StatusWatcher`] for a service
    ///
    /// # Panics
    /// If the service watcher is not available, although this should never
    /// happen.
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

    /// Send a start signal to the specified service.
    ///
    /// # Errors
    ///
    /// If the start signal cannot be successfully delivered to the specified
    /// service.
    pub async fn start_service<Service>(&self) -> Result<(), ServiceError>
    where
        RuntimeServiceId: AsServiceId<Service>,
    {
        info!("Starting service with ID {}", RuntimeServiceId::SERVICE_ID);

        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.send(OverwatchCommand::ServiceLifeCycle(
            ServiceLifeCycleCommand {
                service_id: RuntimeServiceId::SERVICE_ID,
                msg: LifecycleMessage::Start(sender),
            },
        ))
        .await
        .map_err(|e| {
            dbg!(e);
            ServiceError::Start
        })?;
        receiver.await.map_err(|e| {
            dbg!(e);
            ServiceError::Start
        })?;
        Ok(())
    }

    /// Send a start signal to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner) signaling it
    /// to start a list of services.
    ///
    /// # Arguments
    ///
    /// `service_ids` - A list of service IDs to start.
    ///
    /// # Errors
    ///
    /// Fails silently if the start signal cannot be sent.
    pub async fn start_service_list(&self, service_ids: impl Into<Vec<RuntimeServiceId>>) {
        let service_ids: Vec<RuntimeServiceId> = service_ids.into();
        info!("Starting services: {:?}", service_ids);
        let _: Result<(), _> = self
            .send(OverwatchCommand::OverwatchLifeCycle(
                OverwatchLifeCycleCommand::StartServiceList(service_ids),
            ))
            .await
            .map_err(|e| dbg!(e));
    }

    /// Send a start signal to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner) signaling it
    /// to start all services.
    ///
    /// # Errors
    ///
    /// Fails silently if the start signal cannot be sent.
    pub async fn start_all_services(&self) {
        info!("Starting all services");
        let _: Result<(), _> = self
            .send(OverwatchCommand::OverwatchLifeCycle(
                OverwatchLifeCycleCommand::StartAllServices,
            ))
            .await
            .map_err(|e| dbg!(e));
    }

    /// Send a stop signal to the specified service.
    ///
    /// # Errors
    ///
    /// If the stop signal cannot be successfully delivered to the specified
    /// service.
    pub async fn stop_service<Service>(&self) -> Result<(), ServiceError>
    where
        RuntimeServiceId: AsServiceId<Service>,
    {
        info!("Stopping service with ID {}", RuntimeServiceId::SERVICE_ID);

        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.send(OverwatchCommand::ServiceLifeCycle(
            ServiceLifeCycleCommand {
                service_id: RuntimeServiceId::SERVICE_ID,
                msg: LifecycleMessage::Stop(sender),
            },
        ))
        .await
        .map_err(|e| {
            dbg!(e);
            ServiceError::Stop
        })?;
        receiver.await.map_err(|e| {
            dbg!(e);
            ServiceError::Stop
        })?;
        Ok(())
    }

    /// Send a stop signal to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner)
    /// signaling it to stop a list of services.
    ///
    /// # Arguments
    ///
    /// `service_ids` - A list of service IDs to stop.
    ///
    /// # Errors
    ///
    /// Fails silently if the stop signal cannot be sent.
    pub async fn stop_service_list(&self, service_ids: impl Into<Vec<RuntimeServiceId>>) {
        let service_ids: Vec<RuntimeServiceId> = service_ids.into();
        info!("Stopping services: {:?}", service_ids);
        let _: Result<(), _> = self
            .send(OverwatchCommand::OverwatchLifeCycle(
                OverwatchLifeCycleCommand::StopServiceList(service_ids),
            ))
            .await
            .map_err(|e| dbg!(e));
    }

    pub async fn stop_all_services(&self) {
        info!("Stopping all services");
        let _: Result<(), _> = self
            .send(OverwatchCommand::OverwatchLifeCycle(
                OverwatchLifeCycleCommand::StopAllServices,
            ))
            .await
            .map_err(|e| dbg!(e));
    }

    /// Send a shutdown signal to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner) signaling it
    /// to stop all services.
    ///
    /// This triggers sending the `finish_runner_signal` to
    /// [`Overwatch`](crate::overwatch::Overwatch). It's the signal which
    /// [`Overwatch::wait_finished`](crate::overwatch::Overwatch::wait_finished)
    /// waits for.
    ///
    /// # Errors
    ///
    /// Fails silently if the shutdown signal cannot be sent.
    pub async fn shutdown(&self) {
        info!("Shutting down Overwatch");
        let _: Result<(), _> = self
            .send(OverwatchCommand::OverwatchLifeCycle(
                OverwatchLifeCycleCommand::Shutdown,
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
