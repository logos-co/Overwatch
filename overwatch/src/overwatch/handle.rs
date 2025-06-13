use std::fmt::{Debug, Display};

use tokio::{
    runtime::Handle,
    sync::mpsc::{error::SendError, Sender},
};
#[cfg(feature = "instrumentation")]
use tracing::instrument;
use tracing::{debug, error, info};

use crate::{
    overwatch::{
        commands::{
            OverwatchCommand, OverwatchManagementCommand, RelayCommand, ReplyChannel,
            ServiceAllCommand, ServiceLifecycleCommand, ServiceSequenceCommand,
            ServiceSingleCommand, SettingsCommand, StatusCommand,
        },
        errors::OverwatchManagementError,
        Error, Services,
    },
    services::{
        lifecycle::ServiceLifecycleError,
        relay::{OutboundRelay, RelayError},
        status::StatusWatcher,
        AsServiceId, ServiceData,
    },
    utils::finished_signal,
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
    ///
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

    /// Send a [`ServiceLifecycleCommand::StartService`] command to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
    ///
    /// # Arguments
    ///
    /// * `Service` - The service type to start.
    ///
    /// # Errors
    ///
    /// If the command cannot be sent, or if the
    /// [`Signal`](finished_signal::Signal) is not received.
    pub async fn start_service<Service>(&self) -> Result<(), Error>
    where
        RuntimeServiceId: AsServiceId<Service>,
    {
        info!("Starting Service with ID {}", RuntimeServiceId::SERVICE_ID);

        let (sender, receiver) = finished_signal::channel();
        let command = OverwatchCommand::ServiceLifecycle(ServiceLifecycleCommand::StartService(
            ServiceSingleCommand {
                service_id: RuntimeServiceId::SERVICE_ID,
                sender,
            },
        ));

        self.send(command)
            .await
            .map_err(|_error| ServiceLifecycleError::Start)?;

        receiver.await.map_err(|error| {
            debug!("{error:?}");
            ServiceLifecycleError::Start.into()
        })
    }

    /// Send a [`ServiceLifecycleCommand::StartServiceSequence`] command to
    /// the [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
    ///
    /// # Arguments
    ///
    /// * `service_ids` - A list of service IDs to start.
    ///
    /// # Errors
    ///
    /// If the command cannot be sent, or if the
    /// [`Signal`](finished_signal::Signal) is not received.
    pub async fn start_service_sequence(
        &self,
        service_ids: impl IntoIterator<Item = RuntimeServiceId>,
    ) -> Result<(), Error> {
        let service_ids = service_ids.into_iter().collect::<Vec<RuntimeServiceId>>();
        info!("Starting Service Sequence with IDs: {:?}", service_ids);

        let (sender, receiver) = finished_signal::channel();
        let command = OverwatchCommand::ServiceLifecycle(
            ServiceLifecycleCommand::StartServiceSequence(ServiceSequenceCommand {
                service_ids,
                sender,
            }),
        );

        self.send(command)
            .await
            .map_err(|_error| ServiceLifecycleError::StartSequence)?;

        receiver.await.map_err(|error| {
            debug!("{error:?}");
            ServiceLifecycleError::StartSequence.into()
        })
    }

    /// Send a [`ServiceLifecycleCommand::StartAllServices`] command to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
    ///
    /// # Errors
    ///
    /// If the command cannot be sent, or if the
    /// [`Signal`](finished_signal::Signal) is not received.
    pub async fn start_all_services(&self) -> Result<(), Error> {
        info!("Starting all services");

        let (sender, receiver) = finished_signal::channel();
        let command = OverwatchCommand::ServiceLifecycle(
            ServiceLifecycleCommand::StartAllServices(ServiceAllCommand { sender }),
        );

        self.send(command)
            .await
            .map_err(|_error| ServiceLifecycleError::StartAll)?;

        receiver.await.map_err(|error| {
            debug!("{error:?}");
            ServiceLifecycleError::StartAll.into()
        })
    }

    /// Send a [`ServiceLifecycleCommand::StopService`] command to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
    ///
    /// # Arguments
    ///
    /// * `Service` - The service type to stop.
    ///
    /// # Errors
    ///
    /// If the stop signal cannot be sent, or if the
    /// [`Signal`](finished_signal::Signal) is not received.
    pub async fn stop_service<Service>(&self) -> Result<(), Error>
    where
        RuntimeServiceId: AsServiceId<Service>,
    {
        info!("Stopping Service with ID {}", RuntimeServiceId::SERVICE_ID);

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let command = OverwatchCommand::ServiceLifecycle(ServiceLifecycleCommand::StopService(
            ServiceSingleCommand {
                service_id: RuntimeServiceId::SERVICE_ID,
                sender,
            },
        ));

        self.send(command)
            .await
            .map_err(|_error| ServiceLifecycleError::Stop)?;

        receiver.await.map_err(|error| {
            debug!("{error:?}");
            ServiceLifecycleError::Stop.into()
        })
    }

    /// Send a [`ServiceLifecycleCommand::StopServiceSequence`] command to
    /// the [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
    ///
    /// # Arguments
    ///
    /// * `service_ids` - A list of service IDs to stop.
    ///
    /// # Errors
    ///
    /// If the stop signal cannot be sent, or if the
    /// [`Signal`](finished_signal::Signal) is not received.
    pub async fn stop_service_sequence(
        &self,
        service_ids: impl IntoIterator<Item = RuntimeServiceId>,
    ) -> Result<(), Error> {
        let service_ids = service_ids.into_iter().collect::<Vec<RuntimeServiceId>>();
        info!("Stopping Service Sequence with IDs: {:?}", service_ids);

        let (sender, receiver) = finished_signal::channel();
        let command = OverwatchCommand::ServiceLifecycle(
            ServiceLifecycleCommand::StopServiceSequence(ServiceSequenceCommand {
                service_ids,
                sender,
            }),
        );

        self.send(command)
            .await
            .map_err(|_error| ServiceLifecycleError::StopSequence)?;

        receiver.await.map_err(|error| {
            debug!("{error:?}");
            ServiceLifecycleError::StopSequence.into()
        })
    }

    /// Send a [`ServiceLifecycleCommand::StopAllServices`] command to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
    ///
    /// # Errors
    ///
    /// If the command cannot be sent, or if the
    /// [`Signal`](finished_signal::Signal) is not received.
    pub async fn stop_all_services(&self) -> Result<(), Error> {
        info!("Stopping all services");

        let (sender, receiver) = finished_signal::channel();
        let command = OverwatchCommand::ServiceLifecycle(ServiceLifecycleCommand::StopAllServices(
            ServiceAllCommand { sender },
        ));

        self.send(command)
            .await
            .map_err(|_error| ServiceLifecycleError::StopAll)?;

        receiver.await.map_err(|error| {
            debug!("{error:?}");
            ServiceLifecycleError::StopAll.into()
        })
    }

    /// Send a [`ServiceLifecycleCommand::Shutdown`] command to the
    /// [`OverwatchRunner`](crate::overwatch::OverwatchRunner).
    ///
    /// This triggers sending the `finish_runner_signal` to
    /// [`Overwatch`](crate::overwatch::Overwatch). It's the signal which
    /// [`Overwatch::wait_finished`](crate::overwatch::Overwatch::wait_finished)
    /// waits for.
    ///
    /// # Errors
    ///
    /// If the command cannot be sent, or if the
    /// [`Signal`](finished_signal::Signal) is not received.
    pub async fn shutdown(&self) -> Result<(), Error> {
        info!("Shutting down Overwatch");

        let (sender, receiver) = finished_signal::channel();
        let command =
            OverwatchCommand::OverwatchManagement(OverwatchManagementCommand::Shutdown(sender));

        self.send(command)
            .await
            .map_err(|_error| OverwatchManagementError::Shutdown)?;

        receiver.await.map_err(|error| {
            debug!("{error:?}");
            OverwatchManagementError::Shutdown.into()
        })
    }

    /// Retrieve all `Service`'s `RuntimeServiceId`'s.
    ///
    /// # Errors
    ///
    /// If the service IDs cannot be retrieved.
    pub async fn retrieve_service_ids(&self) -> Result<Vec<RuntimeServiceId>, Error> {
        info!("Retrieving all service IDs.");
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let reply_channel = ReplyChannel::from(sender);
        let command = OverwatchCommand::OverwatchManagement(
            OverwatchManagementCommand::RetrieveServiceIds(reply_channel),
        );

        self.send(command)
            .await
            .map_err(|_error| OverwatchManagementError::RetrieveServiceIds)?;

        receiver.await.map_err(|error| {
            error!(error=?error, "Error while retrieving service IDs");
            OverwatchManagementError::RetrieveServiceIds.into()
        })
    }

    /// Send a command to the
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
        self.sender.send(command).await.map_err(|error| {
            error!(error=?error, "Error while sending an Overwatch command");
            error
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
