use std::time::Duration;

use tokio::time::timeout;

use crate::services::status::{Receiver, service_status::ServiceStatus};

/// Error returned by [`StatusWatcher::wait_for`] and
/// [`StatusWatcher::wait_for_fail_fast`].
///
/// `initial_status` is captured when the wait begins executing. Boundary
/// errors also contain the latest status sampled when they are classified.
/// Tokio watch channels can coalesce updates: these are observations, not a
/// complete history of status transitions.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
pub enum WaitForError {
    /// The service stopped while waiting for a different status.
    #[error(
        "service stopped while waiting for the requested status; initial status: {initial_status}"
    )]
    ServiceStopped { initial_status: ServiceStatus },
    /// The service failed while waiting for a different status.
    #[error(
        "service failed while waiting for the requested status; initial status: {initial_status}"
    )]
    ServiceFailed { initial_status: ServiceStatus },
    /// The wait timed out; `latest_status` is the latest status available when
    /// the timeout was classified.
    #[error(
        "timed out waiting for service status; initial status: {initial_status}; latest status: {latest_status}"
    )]
    Timeout {
        initial_status: ServiceStatus,
        latest_status: ServiceStatus,
    },
    /// Status notifications closed before the requested status was reached;
    /// `latest_status` is the latest status available when closure was handled.
    #[error(
        "service status notification channel closed; initial status: {initial_status}; latest status: {latest_status}"
    )]
    ChannelClosed {
        initial_status: ServiceStatus,
        latest_status: ServiceStatus,
    },
}

/// Watcher for the [`ServiceStatus`] updates.
///
/// Waiting uses Tokio watch notifications and is cancelled when the wait
/// future is dropped. At timeout or channel closure, one final status sample
/// is taken: the requested status wins, followed by errors under the selected
/// waiting policy, followed by the timeout or closure error. Watch updates may
/// coalesce, so intermediate overwritten transitions can be missed.
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

    /// Wait for a [`ServiceStatus`], allowing services to start or restart.
    ///
    /// [`ServiceStatus::Stopped`] does not end the wait unless requested.
    /// If the wait begins in [`ServiceStatus::Failed`], later failures also
    /// remain tolerated for the entire wait, including after an intervening
    /// [`ServiceStatus::Starting`].
    ///
    /// `Some(duration)` bounds the wait; `None` imposes no deadline.
    ///
    /// # Errors
    ///
    /// Returns [`WaitForError::ServiceFailed`] for an unexpected
    /// [`ServiceStatus::Failed`] when the initial status was not `Failed`,
    /// [`WaitForError::Timeout`] when the bounded wait expires, or
    /// [`WaitForError::ChannelClosed`] when status notifications close.
    /// All errors include the initial status; boundary errors also include the
    /// latest sampled status.
    ///
    /// # Compatibility
    ///
    /// The error type is now [`WaitForError`] rather than [`ServiceStatus`].
    /// Consumers must update their error matches and propagation types.
    pub async fn wait_for(
        &mut self,
        status: ServiceStatus,
        timeout_duration: Option<Duration>,
    ) -> Result<ServiceStatus, WaitForError> {
        self.wait_for_inner(status, timeout_duration, WaitPolicy::RestartFriendly)
            .await
    }

    /// Wait for a [`ServiceStatus`] unless the service stops or fails.
    ///
    /// When waiting for [`ServiceStatus::Ready`], call this only after the
    /// service's startup boundary has been acknowledged. The current status
    /// alone cannot distinguish a service that has not started yet from one
    /// that has already stopped.
    ///
    /// `Some(duration)` bounds the wait; `None` imposes no deadline.
    ///
    /// At a timeout or channel-closure boundary, the requested status takes
    /// precedence if it is currently visible; otherwise an unexpected
    /// [`ServiceStatus::Stopped`] or [`ServiceStatus::Failed`] takes precedence
    /// over the boundary error. Tokio's watch channel exposes only its latest
    /// status, so intermediate overwritten transitions cannot always be
    /// identified.
    ///
    /// # Errors
    ///
    /// Returns [`WaitForError::ServiceStopped`] or
    /// [`WaitForError::ServiceFailed`] for an unexpected
    /// [`ServiceStatus::Stopped`] or [`ServiceStatus::Failed`],
    /// [`WaitForError::Timeout`] when the bounded wait expires, or
    /// [`WaitForError::ChannelClosed`] when status notifications close before
    /// the requested status is reached. Boundary errors include the latest
    /// status available when they are classified.
    pub async fn wait_for_fail_fast(
        &mut self,
        status: ServiceStatus,
        timeout_duration: Option<Duration>,
    ) -> Result<ServiceStatus, WaitForError> {
        self.wait_for_inner(status, timeout_duration, WaitPolicy::FailFast)
            .await
    }

    async fn wait_for_inner(
        &mut self,
        status: ServiceStatus,
        timeout_duration: Option<Duration>,
        policy: WaitPolicy,
    ) -> Result<ServiceStatus, WaitForError> {
        let initial_status = self.current();
        if status == initial_status {
            return Ok(initial_status);
        }

        if let Some(error) = policy.stopping_error(initial_status, initial_status) {
            return Err(error);
        }

        let wait = self.receiver.wait_for(|observed| {
            observed == &status || policy.stopping_error(initial_status, *observed).is_some()
        });
        let outcome = match timeout_duration {
            Some(timeout_duration) => match timeout(timeout_duration, wait).await {
                Ok(Ok(observed)) => WaitOutcome::Reached(*observed),
                Ok(Err(_)) => WaitOutcome::Boundary(WaitBoundary::ChannelClosed),
                Err(_) => WaitOutcome::Boundary(WaitBoundary::Timeout),
            },
            None => wait.await.map_or(
                WaitOutcome::Boundary(WaitBoundary::ChannelClosed),
                |observed| WaitOutcome::Reached(*observed),
            ),
        };

        match outcome {
            WaitOutcome::Reached(observed) if observed == status => Ok(observed),
            // Classify the observation that ended the wait, not a later update.
            WaitOutcome::Reached(observed) => policy
                .stopping_error(initial_status, observed)
                .map_or(Ok(observed), Err),
            WaitOutcome::Boundary(boundary) => {
                let latest_status = self.current();
                classify_wait_boundary(status, initial_status, latest_status, policy, boundary)
            }
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum WaitOutcome {
    Reached(ServiceStatus),
    Boundary(WaitBoundary),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum WaitPolicy {
    RestartFriendly,
    FailFast,
}

impl WaitPolicy {
    fn stopping_error(
        self,
        initial_status: ServiceStatus,
        observed_status: ServiceStatus,
    ) -> Option<WaitForError> {
        // This rule remains fixed by the initial status for the whole wait.
        let stop_on_failure = initial_status != ServiceStatus::Failed;
        match observed_status {
            ServiceStatus::Stopped if self == Self::FailFast => {
                Some(WaitForError::ServiceStopped { initial_status })
            }
            ServiceStatus::Failed if self == Self::FailFast || stop_on_failure => {
                Some(WaitForError::ServiceFailed { initial_status })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum WaitBoundary {
    Timeout,
    ChannelClosed,
}

fn classify_wait_boundary(
    requested_status: ServiceStatus,
    initial_status: ServiceStatus,
    latest_status: ServiceStatus,
    policy: WaitPolicy,
    boundary: WaitBoundary,
) -> Result<ServiceStatus, WaitForError> {
    if latest_status == requested_status {
        return Ok(latest_status);
    }

    if let Some(error) = policy.stopping_error(initial_status, latest_status) {
        return Err(error);
    }

    match boundary {
        WaitBoundary::Timeout => Err(WaitForError::Timeout {
            initial_status,
            latest_status,
        }),
        WaitBoundary::ChannelClosed => Err(WaitForError::ChannelClosed {
            initial_status,
            latest_status,
        }),
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::watch;

    use super::*;

    fn watcher_with(status: ServiceStatus) -> (StatusWatcher, watch::Sender<ServiceStatus>) {
        let (sender, receiver) = watch::channel(status);
        (StatusWatcher::new(receiver), sender)
    }

    #[tokio::test]
    async fn default_waits_for_ready_from_initially_stopped() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Stopped);
        let wait = watcher.wait_for(ServiceStatus::Ready, None);
        tokio::pin!(wait);

        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Starting);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Ready);

        assert_eq!(wait.await, Ok(ServiceStatus::Ready));
    }

    #[tokio::test]
    async fn default_waits_for_ready_after_initial_failure_and_restart() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Failed);
        let wait = watcher.wait_for(ServiceStatus::Ready, None);
        tokio::pin!(wait);

        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Starting);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Ready);

        assert_eq!(wait.await, Ok(ServiceStatus::Ready));
    }

    #[tokio::test]
    async fn default_waits_through_observed_stop_and_restart() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        let wait = watcher.wait_for(ServiceStatus::Ready, None);
        tokio::pin!(wait);

        assert!(futures::poll!(wait.as_mut()).is_pending());
        for status in [ServiceStatus::Stopped, ServiceStatus::Starting] {
            sender.send_replace(status);
            assert!(futures::poll!(wait.as_mut()).is_pending());
        }
        sender.send_replace(ServiceStatus::Ready);

        assert_eq!(wait.await, Ok(ServiceStatus::Ready));
    }

    #[tokio::test]
    async fn default_wait_tolerates_repeated_failures_after_initial_failure() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Failed);
        let wait = watcher.wait_for(ServiceStatus::Ready, None);
        tokio::pin!(wait);

        assert!(futures::poll!(wait.as_mut()).is_pending());
        for status in [
            ServiceStatus::Starting,
            ServiceStatus::Failed,
            ServiceStatus::Starting,
        ] {
            sender.send_replace(status);
            assert!(futures::poll!(wait.as_mut()).is_pending());
        }
        sender.send_replace(ServiceStatus::Ready);

        assert_eq!(wait.await, Ok(ServiceStatus::Ready));
    }

    #[tokio::test]
    async fn default_wait_can_request_stopped_or_failed() {
        for status in [ServiceStatus::Stopped, ServiceStatus::Failed] {
            let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
            let wait = watcher.wait_for(status, None);
            tokio::pin!(wait);

            assert!(futures::poll!(wait.as_mut()).is_pending());
            sender.send_replace(status);

            assert_eq!(wait.await, Ok(status));
        }
    }

    #[tokio::test]
    async fn both_wait_methods_succeed_when_already_at_requested_status() {
        for status in [
            ServiceStatus::Starting,
            ServiceStatus::Ready,
            ServiceStatus::Stopped,
            ServiceStatus::Failed,
        ] {
            let (mut watcher, _sender) = watcher_with(status);
            assert_eq!(
                watcher.wait_for(status, Some(Duration::ZERO)).await,
                Ok(status)
            );
            assert_eq!(
                watcher
                    .wait_for_fail_fast(status, Some(Duration::ZERO))
                    .await,
                Ok(status)
            );
        }
    }

    #[tokio::test]
    async fn initial_status_is_captured_when_wait_begins_executing() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Stopped);
        let wait = watcher.wait_for_fail_fast(ServiceStatus::Ready, None);
        tokio::pin!(wait);

        sender.send_replace(ServiceStatus::Starting);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Failed);

        assert_eq!(
            wait.await,
            Err(WaitForError::ServiceFailed {
                initial_status: ServiceStatus::Starting,
            })
        );
    }

    #[tokio::test]
    async fn default_reports_new_failure_transition() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        let wait = watcher.wait_for(ServiceStatus::Ready, None);
        tokio::pin!(wait);

        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Failed);

        assert_eq!(
            wait.await,
            Err(WaitForError::ServiceFailed {
                initial_status: ServiceStatus::Starting,
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn default_timeout_reports_initial_and_latest_status() {
        for (initial_status, latest_status) in [
            (ServiceStatus::Stopped, ServiceStatus::Starting),
            (ServiceStatus::Starting, ServiceStatus::Stopped),
            (ServiceStatus::Failed, ServiceStatus::Failed),
        ] {
            let (mut watcher, sender) = watcher_with(initial_status);
            let wait = watcher.wait_for(ServiceStatus::Ready, Some(Duration::from_secs(5)));
            tokio::pin!(wait);

            assert!(futures::poll!(wait.as_mut()).is_pending());
            sender.send_replace(ServiceStatus::Starting);
            assert!(futures::poll!(wait.as_mut()).is_pending());
            sender.send_replace(latest_status);
            assert!(futures::poll!(wait.as_mut()).is_pending());
            tokio::time::advance(Duration::from_secs(5)).await;

            assert_eq!(
                wait.await,
                Err(WaitForError::Timeout {
                    initial_status,
                    latest_status,
                })
            );
        }
    }

    #[tokio::test]
    async fn default_channel_closure_reports_initial_and_latest_status() {
        for (initial_status, latest_status) in [
            (ServiceStatus::Failed, ServiceStatus::Starting),
            (ServiceStatus::Starting, ServiceStatus::Stopped),
            (ServiceStatus::Failed, ServiceStatus::Failed),
        ] {
            let (mut watcher, sender) = watcher_with(initial_status);
            let wait = watcher.wait_for(ServiceStatus::Ready, None);
            tokio::pin!(wait);

            assert!(futures::poll!(wait.as_mut()).is_pending());
            sender.send_replace(ServiceStatus::Starting);
            assert!(futures::poll!(wait.as_mut()).is_pending());
            sender.send_replace(latest_status);
            assert!(futures::poll!(wait.as_mut()).is_pending());
            drop(sender);

            assert_eq!(
                wait.await,
                Err(WaitForError::ChannelClosed {
                    initial_status,
                    latest_status,
                })
            );
        }
    }

    #[tokio::test]
    async fn wait_fails_immediately_if_initially_stopped_or_failed() {
        for (status, error) in [
            (
                ServiceStatus::Stopped,
                WaitForError::ServiceStopped {
                    initial_status: ServiceStatus::Stopped,
                },
            ),
            (
                ServiceStatus::Failed,
                WaitForError::ServiceFailed {
                    initial_status: ServiceStatus::Failed,
                },
            ),
        ] {
            let (mut watcher, _sender) = watcher_with(status);
            assert_eq!(
                watcher.wait_for_fail_fast(ServiceStatus::Ready, None).await,
                Err(error)
            );
        }
    }

    #[tokio::test]
    async fn wait_reports_stopped_or_failed_transitions() {
        for (status, error) in [
            (
                ServiceStatus::Stopped,
                WaitForError::ServiceStopped {
                    initial_status: ServiceStatus::Starting,
                },
            ),
            (
                ServiceStatus::Failed,
                WaitForError::ServiceFailed {
                    initial_status: ServiceStatus::Starting,
                },
            ),
        ] {
            let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
            let wait = watcher.wait_for_fail_fast(ServiceStatus::Ready, None);
            tokio::pin!(wait);

            assert!(futures::poll!(wait.as_mut()).is_pending());
            sender.send_replace(status);

            let result = wait.await;
            // A later update must not replace the observation that ended the wait.
            sender.send_replace(ServiceStatus::Ready);
            assert_eq!(result, Err(error));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn wait_timeout_reports_latest_status() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        let wait = watcher.wait_for_fail_fast(ServiceStatus::Stopped, Some(Duration::from_secs(5)));
        tokio::pin!(wait);

        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        tokio::time::advance(Duration::from_secs(5)).await;

        assert_eq!(
            wait.await,
            Err(WaitForError::Timeout {
                initial_status: ServiceStatus::Starting,
                latest_status: ServiceStatus::Ready,
            })
        );
    }

    #[tokio::test]
    async fn wait_reports_latest_status_when_channel_closes() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        let wait = watcher.wait_for_fail_fast(ServiceStatus::Stopped, None);
        tokio::pin!(wait);

        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        drop(sender);

        assert_eq!(
            wait.await,
            Err(WaitForError::ChannelClosed {
                initial_status: ServiceStatus::Starting,
                latest_status: ServiceStatus::Ready,
            })
        );
    }

    #[test]
    fn boundary_prefers_requested_status_before_stopped_or_failed_errors() {
        for boundary in [WaitBoundary::Timeout, WaitBoundary::ChannelClosed] {
            for policy in [WaitPolicy::RestartFriendly, WaitPolicy::FailFast] {
                for requested in [
                    ServiceStatus::Starting,
                    ServiceStatus::Ready,
                    ServiceStatus::Stopped,
                    ServiceStatus::Failed,
                ] {
                    assert_eq!(
                        classify_wait_boundary(
                            requested,
                            ServiceStatus::Starting,
                            requested,
                            policy,
                            boundary,
                        ),
                        Ok(requested)
                    );
                }
            }

            for (latest_status, error) in [
                (
                    ServiceStatus::Stopped,
                    WaitForError::ServiceStopped {
                        initial_status: ServiceStatus::Starting,
                    },
                ),
                (
                    ServiceStatus::Failed,
                    WaitForError::ServiceFailed {
                        initial_status: ServiceStatus::Starting,
                    },
                ),
            ] {
                assert_eq!(
                    classify_wait_boundary(
                        ServiceStatus::Ready,
                        ServiceStatus::Starting,
                        latest_status,
                        WaitPolicy::FailFast,
                        boundary,
                    ),
                    Err(error)
                );
            }
        }
    }

    #[test]
    fn default_boundaries_preserve_restart_policy() {
        for boundary in [WaitBoundary::Timeout, WaitBoundary::ChannelClosed] {
            for (initial_status, latest_status) in [
                (ServiceStatus::Stopped, ServiceStatus::Starting),
                (ServiceStatus::Starting, ServiceStatus::Stopped),
                (ServiceStatus::Failed, ServiceStatus::Failed),
                (ServiceStatus::Failed, ServiceStatus::Starting),
            ] {
                let error = match boundary {
                    WaitBoundary::Timeout => WaitForError::Timeout {
                        initial_status,
                        latest_status,
                    },
                    WaitBoundary::ChannelClosed => WaitForError::ChannelClosed {
                        initial_status,
                        latest_status,
                    },
                };
                assert_eq!(
                    classify_wait_boundary(
                        ServiceStatus::Ready,
                        initial_status,
                        latest_status,
                        WaitPolicy::RestartFriendly,
                        boundary,
                    ),
                    Err(error)
                );
            }

            assert_eq!(
                classify_wait_boundary(
                    ServiceStatus::Ready,
                    ServiceStatus::Starting,
                    ServiceStatus::Failed,
                    WaitPolicy::RestartFriendly,
                    boundary,
                ),
                Err(WaitForError::ServiceFailed {
                    initial_status: ServiceStatus::Starting,
                })
            );
        }
    }

    #[tokio::test]
    async fn wait_succeeds_when_starting_becomes_ready() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        let wait =
            tokio::spawn(
                async move { watcher.wait_for_fail_fast(ServiceStatus::Ready, None).await },
            );

        tokio::task::yield_now().await;
        sender.send_replace(ServiceStatus::Ready);

        assert_eq!(wait.await.unwrap(), Ok(ServiceStatus::Ready));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_succeeds_before_bounded_deadline() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            sender.send_replace(ServiceStatus::Ready);
        });

        assert_eq!(
            watcher
                .wait_for_fail_fast(ServiceStatus::Ready, Some(Duration::from_secs(10)))
                .await,
            Ok(ServiceStatus::Ready)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_wait_times_out_while_starting() {
        let (mut watcher, _sender) = watcher_with(ServiceStatus::Starting);

        assert_eq!(
            watcher
                .wait_for_fail_fast(ServiceStatus::Ready, Some(Duration::from_secs(5)))
                .await,
            Err(WaitForError::Timeout {
                initial_status: ServiceStatus::Starting,
                latest_status: ServiceStatus::Starting,
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn unbounded_wait_succeeds_after_more_than_sixty_seconds() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        let mut restart_friendly_watcher = watcher.clone();
        let notify_ready = async move {
            tokio::time::sleep(Duration::from_secs(61)).await;
            sender.send_replace(ServiceStatus::Ready);
        };

        let (restart_friendly_result, fail_fast_result, ()) = tokio::join!(
            restart_friendly_watcher.wait_for(ServiceStatus::Ready, None),
            watcher.wait_for_fail_fast(ServiceStatus::Ready, None),
            notify_ready,
        );
        assert_eq!(restart_friendly_result, Ok(ServiceStatus::Ready));
        assert_eq!(fail_fast_result, Ok(ServiceStatus::Ready));
    }

    #[tokio::test]
    async fn wait_can_request_stopped_or_failed() {
        let (mut stopped_watcher, _sender) = watcher_with(ServiceStatus::Stopped);
        assert_eq!(
            stopped_watcher
                .wait_for_fail_fast(ServiceStatus::Stopped, None)
                .await,
            Ok(ServiceStatus::Stopped)
        );

        let (mut failed_watcher, _sender) = watcher_with(ServiceStatus::Failed);
        assert_eq!(
            failed_watcher
                .wait_for_fail_fast(ServiceStatus::Failed, None)
                .await,
            Ok(ServiceStatus::Failed)
        );

        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        let wait = tokio::spawn(async move {
            watcher
                .wait_for_fail_fast(ServiceStatus::Stopped, None)
                .await
        });
        tokio::task::yield_now().await;
        sender.send_replace(ServiceStatus::Stopped);
        assert_eq!(wait.await.unwrap(), Ok(ServiceStatus::Stopped));
    }

    #[tokio::test]
    async fn closed_status_notifications_return_an_error() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        drop(sender);

        assert_eq!(
            timeout(
                Duration::from_secs(1),
                watcher.wait_for_fail_fast(ServiceStatus::Ready, None)
            )
            .await
            .unwrap(),
            Err(WaitForError::ChannelClosed {
                initial_status: ServiceStatus::Starting,
                latest_status: ServiceStatus::Starting,
            })
        );
    }

    #[tokio::test]
    async fn dropping_pending_waits_leaves_watchers_reusable() {
        let (mut watcher, sender) = watcher_with(ServiceStatus::Starting);
        let mut fail_fast_watcher = watcher.clone();
        let mut wait = Box::pin(watcher.wait_for(ServiceStatus::Ready, None));
        let mut fail_fast_wait =
            Box::pin(fail_fast_watcher.wait_for_fail_fast(ServiceStatus::Ready, None));

        assert!(futures::poll!(wait.as_mut()).is_pending());
        assert!(futures::poll!(fail_fast_wait.as_mut()).is_pending());
        drop(wait);
        drop(fail_fast_wait);
        sender.send_replace(ServiceStatus::Ready);

        assert_eq!(
            watcher.wait_for(ServiceStatus::Ready, None).await,
            Ok(ServiceStatus::Ready)
        );
        assert_eq!(
            fail_fast_watcher
                .wait_for_fail_fast(ServiceStatus::Ready, None)
                .await,
            Ok(ServiceStatus::Ready)
        );
    }

    #[test]
    fn errors_display_initial_status_and_failure_context() {
        for (error, initial_status, reason) in [
            (
                WaitForError::ServiceStopped {
                    initial_status: ServiceStatus::Starting,
                },
                ServiceStatus::Starting,
                "service stopped",
            ),
            (
                WaitForError::ServiceFailed {
                    initial_status: ServiceStatus::Starting,
                },
                ServiceStatus::Starting,
                "service failed",
            ),
            (
                WaitForError::Timeout {
                    initial_status: ServiceStatus::Stopped,
                    latest_status: ServiceStatus::Starting,
                },
                ServiceStatus::Stopped,
                "timed out",
            ),
            (
                WaitForError::ChannelClosed {
                    initial_status: ServiceStatus::Failed,
                    latest_status: ServiceStatus::Ready,
                },
                ServiceStatus::Failed,
                "channel closed",
            ),
        ] {
            let message = error.to_string();
            assert!(message.contains(&initial_status.to_string()));
            assert!(message.contains(reason));
            if let WaitForError::Timeout { latest_status, .. }
            | WaitForError::ChannelClosed { latest_status, .. } = error
            {
                assert!(message.contains(&latest_status.to_string()));
            }
        }
    }
}
