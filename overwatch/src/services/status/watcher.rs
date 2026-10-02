use std::time::Duration;

use crate::services::status::{Receiver, service_status::ServiceStatus};

/// Watcher for the [`ServiceStatus`] updates.
#[derive(Debug, Clone)]
pub struct StatusWatcher {
    receiver: Receiver,
}

impl StatusWatcher {
    /// Create a new [`StatusWatcher`].
    #[must_use]
    pub const fn new(receiver: Receiver) -> Self {
        Self { receiver }
    }

    /// Wait for a new [`ServiceStatus`] message.
    ///
    /// # Errors
    ///
    /// If the message is not received within the specified timeout period, or
    /// if the `Service` reaches [`ServiceStatus::Failed`] while waiting for a
    /// different status. The error is the last status seen.
    ///
    /// A `Service` that was already [`ServiceStatus::Failed`] when this is
    /// called is not an error by itself, so that waiting for a restart works.
    pub async fn wait_for(
        &mut self,
        status: ServiceStatus,
        timeout_duration: Option<Duration>,
    ) -> Result<ServiceStatus, ServiceStatus> {
        let current = self.current();
        if status == current {
            return Ok(current);
        }
        let timeout_duration = timeout_duration.unwrap_or_else(|| Duration::from_secs(u64::MAX));
        // A failure that predates this call is not reported: the `Service` might be
        // about to be restarted.
        let stop_on_failure = current != ServiceStatus::Failed;
        let reached = tokio::time::timeout(
            timeout_duration,
            self.receiver
                .wait_for(|s| s == &status || (stop_on_failure && s == &ServiceStatus::Failed)),
        )
        .await
        .map_or(Err(current), |r| r.map(|s| *s).map_err(|_| current))?;
        if reached == status {
            Ok(reached)
        } else {
            Err(reached)
        }
    }

    #[must_use]
    pub fn current(&self) -> ServiceStatus {
        *self.receiver.borrow()
    }

    #[must_use]
    pub const fn receiver_mut(&mut self) -> &mut Receiver {
        &mut self.receiver
    }
}
