use std::{error::Error, fmt, time::Duration};

use futures::future::try_join_all;
use tokio::time::Instant;

use super::{ServiceStatus, StatusWatcher, WaitForError, WaitPolicy};

/// A dependency's status sampled after an aggregate wait failed.
///
/// Samples are taken separately and may differ from the status that ended a
/// wait. Only the dependency whose wait returned an error has a `wait_error`.
#[derive(Debug)]
pub struct DependencyObservation<Id> {
    id: Id,
    status: ServiceStatus,
    wait_error: Option<WaitForError>,
}

impl<Id> DependencyObservation<Id> {
    /// The identifier supplied for this dependency.
    pub const fn id(&self) -> &Id {
        &self.id
    }

    /// The latest status observed when diagnostics were collected.
    pub const fn status(&self) -> ServiceStatus {
        self.status
    }

    /// The original wait error, present only for the failed wait.
    pub const fn wait_error(&self) -> Option<&WaitForError> {
        self.wait_error.as_ref()
    }
}

/// The first error returned by a dependency wait, with ordered status samples.
#[derive(Debug)]
pub struct WaitForAllError<Id> {
    observations: Vec<DependencyObservation<Id>>,
    kind: WaitForAllErrorKind,
    failed_index: Option<usize>,
}

/// Identifies why an aggregate wait returned an error.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WaitForAllErrorKind {
    /// An active individual dependency wait returned an error.
    DependencyWait,
    /// Individual waits succeeded, but final status verification found a
    /// dependency away from the requested target.
    TargetMismatch { target: ServiceStatus },
}

impl<Id> WaitForAllError<Id> {
    /// Observations in the same order as the supplied dependencies.
    #[must_use]
    pub fn observations(&self) -> &[DependencyObservation<Id>] {
        &self.observations
    }

    /// The dependency whose active wait returned an error, if any.
    #[must_use]
    pub fn failed(&self) -> Option<&DependencyObservation<Id>> {
        self.failed_index.map(|index| &self.observations[index])
    }

    /// The original error returned by an individual dependency wait, if any.
    #[must_use]
    pub fn wait_error(&self) -> Option<&WaitForError> {
        self.failed()?.wait_error()
    }

    /// The reason the aggregate wait returned an error.
    #[must_use]
    pub const fn kind(&self) -> WaitForAllErrorKind {
        self.kind
    }

    /// The requested status for a final-verification mismatch.
    #[must_use]
    pub const fn target(&self) -> Option<ServiceStatus> {
        match self.kind {
            WaitForAllErrorKind::TargetMismatch { target } => Some(target),
            WaitForAllErrorKind::DependencyWait => None,
        }
    }
}

impl<Id: fmt::Display> fmt::Display for WaitForAllError<Id> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            WaitForAllErrorKind::DependencyWait => write!(f, "dependency wait failed: ")?,
            WaitForAllErrorKind::TargetMismatch { target } => {
                write!(f, "dependencies not all at target {target}: ")?;
            }
        }
        for (index, observation) in self.observations.iter().enumerate() {
            if index != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", observation.id, observation.status)?;
            if let Some(error) = &observation.wait_error {
                write!(f, " ({error})")?;
            }
        }
        Ok(())
    }
}

impl<Id: fmt::Debug + fmt::Display> Error for WaitForAllError<Id> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.wait_error()
            .map(|error| error as &(dyn Error + 'static))
    }
}

/// Controls when a successful aggregate wait completes.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CompletionPolicy {
    /// Complete once every dependency has reached the target at least once,
    /// without rechecking dependencies whose individual waits have completed.
    TargetReachedOnce,
    /// Complete only after final status samples all match the target. A
    /// mismatch returns immediately under [`WaitPolicy::FailFast`] and causes
    /// only mismatched dependencies to be retried under
    /// [`WaitPolicy::RestartFriendly`].
    AllAtTarget,
}

/// Wait concurrently for every dependency to reach `target`.
///
/// Individual waits use [`StatusWatcher::wait_with_policy`].
/// `TargetReachedOnce` preserves per-dependency timeout behavior. `AllAtTarget`
/// shares a fixed timeout budget across initial and retry waits, then verifies
/// sampled current statuses. With `RestartFriendly`, mismatches are retried
/// until all samples match; with `FailFast`, a mismatch is returned
/// immediately.
///
/// Final verification samples are not atomic. Success means each dependency's
/// latest status matched the target when it was sampled; statuses may change
/// immediately afterward. Dropping this future cancels its active waits and
/// releases the supplied watchers. An empty dependency list succeeds
/// immediately.
///
/// # Errors
///
/// Returns a [`WaitForAllError`] containing either the first individual
/// [`WaitForError`] and observations for every dependency, or a final target
/// mismatch with the target and observed statuses. A mismatch has no
/// individual error source.
pub async fn wait_for_all_with_policy<Id>(
    mut dependencies: Vec<(Id, StatusWatcher)>,
    target: ServiceStatus,
    timeout: Option<Duration>,
    policy: WaitPolicy,
    completion: CompletionPolicy,
) -> Result<(), WaitForAllError<Id>> {
    let deadline = (completion == CompletionPolicy::AllAtTarget)
        .then(|| timeout.map(|duration| Instant::now() + duration))
        .flatten();

    let mut retry_only = None;
    loop {
        let wait_timeout = match completion {
            CompletionPolicy::TargetReachedOnce => timeout,
            CompletionPolicy::AllAtTarget => {
                deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()))
            }
        };
        if let Err((failed_index, error)) = wait_for_dependencies(
            &mut dependencies,
            target,
            wait_timeout,
            policy,
            retry_only.as_deref(),
        )
        .await
        {
            return Err(WaitForAllError {
                observations: collect_observations(dependencies, Some((failed_index, error))),
                kind: WaitForAllErrorKind::DependencyWait,
                failed_index: Some(failed_index),
            });
        }

        if completion == CompletionPolicy::TargetReachedOnce {
            return Ok(());
        }

        let observed_statuses: Vec<_> = dependencies
            .iter()
            .map(|(_, watcher)| watcher.current())
            .collect();
        let mismatches: Vec<_> = observed_statuses
            .iter()
            .map(|status| *status != target)
            .collect();
        if mismatches.iter().all(|mismatch| !mismatch) {
            return Ok(());
        }
        if policy == WaitPolicy::FailFast {
            let observations = dependencies
                .into_iter()
                .zip(observed_statuses)
                .map(|((id, _), status)| DependencyObservation {
                    id,
                    status,
                    wait_error: None,
                })
                .collect();
            return Err(WaitForAllError {
                observations,
                kind: WaitForAllErrorKind::TargetMismatch { target },
                failed_index: None,
            });
        }
        retry_only = Some(mismatches);
    }
}

async fn wait_for_dependencies<Id>(
    dependencies: &mut [(Id, StatusWatcher)],
    target: ServiceStatus,
    timeout: Option<Duration>,
    policy: WaitPolicy,
    retry_only: Option<&[bool]>,
) -> Result<(), (usize, WaitForError)> {
    let waits = dependencies
        .iter_mut()
        .enumerate()
        .filter(|(index, _)| retry_only.is_none_or(|selected| selected[*index]))
        .map(|(index, (_, watcher))| async move {
            watcher
                .wait_with_policy(target, timeout, policy)
                .await
                .map_err(|error| (index, error))
        });
    try_join_all(waits).await.map(|_| ())
}

fn collect_observations<Id>(
    dependencies: Vec<(Id, StatusWatcher)>,
    failed: Option<(usize, WaitForError)>,
) -> Vec<DependencyObservation<Id>> {
    dependencies
        .into_iter()
        .enumerate()
        .map(|(index, (id, watcher))| DependencyObservation {
            id,
            status: watcher.current(),
            wait_error: failed
                .filter(|(failed_index, _)| *failed_index == index)
                .map(|(_, error)| error),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use tokio::sync::watch;

    use super::*;

    fn dependency(status: ServiceStatus) -> (StatusWatcher, watch::Sender<ServiceStatus>) {
        let (sender, receiver) = watch::channel(status);
        (StatusWatcher::new(receiver), sender)
    }

    #[derive(Debug)]
    struct LocalId {
        _local: Rc<()>,
    }

    #[tokio::test]
    async fn all_ready_immediately_or_after_updates() {
        let (ready, _ready_sender) = dependency(ServiceStatus::Ready);
        let (pending, sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![
                (
                    LocalId {
                        _local: Rc::new(()),
                    },
                    ready,
                ),
                (
                    LocalId {
                        _local: Rc::new(()),
                    },
                    pending,
                ),
            ],
            ServiceStatus::Ready,
            None,
            WaitPolicy::FailFast,
            CompletionPolicy::TargetReachedOnce,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Ready);
        assert!(wait.await.is_ok());
        let (ready, _sender) = dependency(ServiceStatus::Ready);
        assert!(
            wait_for_all_with_policy(
                vec![(1, ready)],
                ServiceStatus::Ready,
                Some(Duration::ZERO),
                WaitPolicy::FailFast,
                CompletionPolicy::TargetReachedOnce,
            )
            .await
            .is_ok()
        );
        assert!(
            wait_for_all_with_policy::<()>(
                vec![],
                ServiceStatus::Ready,
                None,
                WaitPolicy::FailFast,
                CompletionPolicy::TargetReachedOnce
            )
            .await
            .is_ok()
        );
    }

    #[tokio::test]
    async fn target_reached_once_ignores_later_departures_for_both_wait_policies() {
        for policy in [WaitPolicy::FailFast, WaitPolicy::RestartFriendly] {
            let (first, first_sender) = dependency(ServiceStatus::Starting);
            let (second, second_sender) = dependency(ServiceStatus::Starting);
            let wait = wait_for_all_with_policy(
                vec![("first", first), ("second", second)],
                ServiceStatus::Ready,
                None,
                policy,
                CompletionPolicy::TargetReachedOnce,
            );
            tokio::pin!(wait);
            assert!(futures::poll!(wait.as_mut()).is_pending());
            first_sender.send_replace(ServiceStatus::Ready);
            assert!(futures::poll!(wait.as_mut()).is_pending());
            first_sender.send_replace(ServiceStatus::Stopped);
            second_sender.send_replace(ServiceStatus::Ready);
            assert!(wait.await.is_ok());
        }
    }

    #[tokio::test]
    async fn fail_fast_all_at_target_reports_final_mismatch_without_wait_error() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![("first", first), ("second", second)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::FailFast,
            CompletionPolicy::AllAtTarget,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Stopped);
        second_sender.send_replace(ServiceStatus::Ready);
        let error = wait.await.unwrap_err();
        assert_eq!(
            error.kind(),
            WaitForAllErrorKind::TargetMismatch {
                target: ServiceStatus::Ready
            }
        );
        assert_eq!(error.target(), Some(ServiceStatus::Ready));
        assert!(error.failed().is_none());
        assert!(error.wait_error().is_none());
        assert!(Error::source(&error).is_none());
        assert_eq!(error.observations().len(), 2);
        assert_eq!(*error.observations()[0].id(), "first");
        assert_eq!(error.observations()[0].status(), ServiceStatus::Stopped);
        assert_eq!(error.observations()[0].wait_error(), None);
        assert_eq!(error.observations()[1].status(), ServiceStatus::Ready);
        assert_eq!(error.observations()[1].wait_error(), None);
    }

    #[tokio::test]
    async fn fail_fast_all_at_target_succeeds_if_dependency_recovers_before_verification() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![(1, first), (2, second)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::FailFast,
            CompletionPolicy::AllAtTarget,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Stopped);
        first_sender.send_replace(ServiceStatus::Ready);
        second_sender.send_replace(ServiceStatus::Ready);
        assert!(wait.await.is_ok());
    }

    #[tokio::test]
    async fn restart_friendly_all_at_target_retries_mismatches() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![("first", first), ("second", second)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::RestartFriendly,
            CompletionPolicy::AllAtTarget,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Stopped);
        second_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Starting);
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(wait.await.is_ok());
    }

    #[tokio::test]
    async fn restart_friendly_all_at_target_rechecks_all_dependencies_each_round() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let (third, third_sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![(1, first), (2, second), (3, third)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::RestartFriendly,
            CompletionPolicy::AllAtTarget,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Stopped);
        second_sender.send_replace(ServiceStatus::Ready);
        third_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());

        // Retry the first dependency while the second leaves the target.
        second_sender.send_replace(ServiceStatus::Stopped);
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());

        // The first dependency is completed for this round, so its departure
        // is found after the second dependency's retry completes.
        first_sender.send_replace(ServiceStatus::Stopped);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        second_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(wait.await.is_ok());
    }

    #[tokio::test]
    async fn retry_wait_failure_preserves_restart_friendly_classification() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![("first", first), ("second", second)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::RestartFriendly,
            CompletionPolicy::AllAtTarget,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Stopped);
        second_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Failed);
        let error = wait.await.unwrap_err();
        assert_eq!(error.kind(), WaitForAllErrorKind::DependencyWait);
        assert_eq!(
            error.wait_error(),
            Some(&WaitForError::ServiceFailed {
                initial_status: ServiceStatus::Stopped
            })
        );
        assert!(Error::source(&error).is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn retry_timeout_uses_the_original_aggregate_deadline() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![("first", first), ("second", second)],
            ServiceStatus::Ready,
            Some(Duration::from_secs(5)),
            WaitPolicy::RestartFriendly,
            CompletionPolicy::AllAtTarget,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Stopped);
        tokio::time::advance(Duration::from_secs(4)).await;
        second_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        tokio::time::advance(Duration::from_secs(1)).await;
        let error = wait.await.unwrap_err();
        assert_eq!(error.kind(), WaitForAllErrorKind::DependencyWait);
        assert_eq!(
            error.wait_error(),
            Some(&WaitForError::Timeout {
                initial_status: ServiceStatus::Stopped,
                latest_status: ServiceStatus::Stopped,
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn unbounded_all_at_target_recovers_after_a_long_restart() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![(1, first), (2, second)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::RestartFriendly,
            CompletionPolicy::AllAtTarget,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Stopped);
        second_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        tokio::time::advance(Duration::from_secs(3600)).await;
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(wait.await.is_ok());
    }

    #[tokio::test]
    async fn dropping_aggregate_during_retry_releases_all_watchers() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let mut wait = Box::pin(wait_for_all_with_policy(
            vec![(1, first), (2, second)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::RestartFriendly,
            CompletionPolicy::AllAtTarget,
        ));
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        first_sender.send_replace(ServiceStatus::Stopped);
        second_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        assert_eq!(first_sender.receiver_count(), 1);
        assert_eq!(second_sender.receiver_count(), 1);
        drop(wait);
        assert_eq!(first_sender.receiver_count(), 0);
        assert_eq!(second_sender.receiver_count(), 0);
    }

    #[tokio::test]
    async fn empty_dependencies_succeed_with_either_completion_policy() {
        for completion in [
            CompletionPolicy::TargetReachedOnce,
            CompletionPolicy::AllAtTarget,
        ] {
            assert!(
                wait_for_all_with_policy::<()>(
                    vec![],
                    ServiceStatus::Ready,
                    None,
                    WaitPolicy::RestartFriendly,
                    completion,
                )
                .await
                .is_ok()
            );
        }
    }

    #[tokio::test]
    async fn failed_wait_returns_promptly_with_ordered_snapshots_and_source() {
        let (ready, _ready_sender) = dependency(ServiceStatus::Ready);
        let (pending, pending_sender) = dependency(ServiceStatus::Starting);
        let (failing, failing_sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![("ready", ready), ("failing", failing), ("pending", pending)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::RestartFriendly,
            CompletionPolicy::TargetReachedOnce,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        failing_sender.send_replace(ServiceStatus::Failed);
        let error = match futures::poll!(wait.as_mut()) {
            std::task::Poll::Ready(Err(error)) => error,
            other => panic!("expected immediate failure, got {other:?}"),
        };
        let expected = WaitForError::ServiceFailed {
            initial_status: ServiceStatus::Starting,
        };
        assert_eq!(*error.failed().unwrap().id(), "failing");
        assert_eq!(*error.wait_error().unwrap(), expected);
        assert_eq!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<WaitForError>()),
            Some(&expected)
        );
        let entries: Vec<_> = error
            .observations()
            .iter()
            .map(|entry| (*entry.id(), entry.status(), entry.wait_error().copied()))
            .collect();
        assert_eq!(
            entries,
            vec![
                ("ready", ServiceStatus::Ready, None),
                ("failing", ServiceStatus::Failed, Some(expected)),
                ("pending", ServiceStatus::Starting, None),
            ]
        );
        assert_eq!(pending_sender.receiver_count(), 0);
        assert_eq!(failing_sender.receiver_count(), 0);
    }

    #[tokio::test]
    async fn restart_friendly_allows_stopped_and_initially_failed_to_restart() {
        let (stopped, stopped_sender) = dependency(ServiceStatus::Stopped);
        let (failed, failed_sender) = dependency(ServiceStatus::Failed);
        let wait = wait_for_all_with_policy(
            vec![("stopped", stopped), ("failed", failed)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::RestartFriendly,
            CompletionPolicy::TargetReachedOnce,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        stopped_sender.send_replace(ServiceStatus::Starting);
        failed_sender.send_replace(ServiceStatus::Starting);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        stopped_sender.send_replace(ServiceStatus::Ready);
        failed_sender.send_replace(ServiceStatus::Ready);
        assert!(wait.await.is_ok());
    }

    #[tokio::test]
    async fn fail_fast_reports_initial_stopped_or_failed() {
        for (status, expected) in [
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
            let (watcher, _sender) = dependency(status);
            let error = wait_for_all_with_policy(
                vec![("dependency", watcher)],
                ServiceStatus::Ready,
                None,
                WaitPolicy::FailFast,
                CompletionPolicy::TargetReachedOnce,
            )
            .await
            .unwrap_err();
            assert_eq!(*error.wait_error().unwrap(), expected);
            assert_eq!(error.failed().unwrap().status(), status);
        }
    }

    #[tokio::test]
    async fn snapshots_are_not_additional_failure_verdicts() {
        let (first, _first_sender) = dependency(ServiceStatus::Stopped);
        let (second, _second_sender) = dependency(ServiceStatus::Failed);
        let error = wait_for_all_with_policy(
            vec![("first", first), ("second", second)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::FailFast,
            CompletionPolicy::TargetReachedOnce,
        )
        .await
        .unwrap_err();
        assert_eq!(*error.failed().unwrap().id(), "first");
        assert_eq!(error.observations()[1].status(), ServiceStatus::Failed);
        assert_eq!(error.observations()[1].wait_error(), None);
    }

    #[tokio::test]
    async fn requested_terminal_status_is_success() {
        for target in [ServiceStatus::Stopped, ServiceStatus::Failed] {
            let (watcher, _sender) = dependency(target);
            assert!(
                wait_for_all_with_policy(
                    vec![(1, watcher)],
                    target,
                    None,
                    WaitPolicy::FailFast,
                    CompletionPolicy::TargetReachedOnce,
                )
                .await
                .is_ok()
            );
        }
    }

    #[tokio::test]
    async fn dropping_aggregate_releases_all_watchers() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let mut wait = Box::pin(wait_for_all_with_policy(
            vec![(1, first), (2, second)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::FailFast,
            CompletionPolicy::TargetReachedOnce,
        ));
        assert!(futures::poll!(wait.as_mut()).is_pending());
        assert_eq!(first_sender.receiver_count(), 1);
        assert_eq!(second_sender.receiver_count(), 1);
        drop(wait);
        assert_eq!(first_sender.receiver_count(), 0);
        assert_eq!(second_sender.receiver_count(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn no_timeout_allows_a_long_wait() {
        let (watcher, sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![(1, watcher)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::FailFast,
            CompletionPolicy::TargetReachedOnce,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        tokio::time::advance(Duration::from_secs(3600)).await;
        assert!(futures::poll!(wait.as_mut()).is_pending());
        sender.send_replace(ServiceStatus::Ready);
        assert!(wait.await.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn timeout_is_per_dependency_and_preserves_original_error() {
        let (ready, ready_sender) = dependency(ServiceStatus::Starting);
        let (pending, _sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![("ready", ready), ("pending", pending)],
            ServiceStatus::Ready,
            Some(Duration::from_secs(5)),
            WaitPolicy::FailFast,
            CompletionPolicy::TargetReachedOnce,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        tokio::time::advance(Duration::from_secs(4)).await;
        ready_sender.send_replace(ServiceStatus::Ready);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        tokio::time::advance(Duration::from_secs(1)).await;
        let error = wait.await.unwrap_err();
        assert_eq!(*error.failed().unwrap().id(), "pending");
        assert_eq!(error.observations()[0].status(), ServiceStatus::Ready);
        assert_eq!(
            *error.wait_error().unwrap(),
            WaitForError::Timeout {
                initial_status: ServiceStatus::Starting,
                latest_status: ServiceStatus::Starting
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn all_dependencies_ready_before_deadline() {
        let (first, first_sender) = dependency(ServiceStatus::Starting);
        let (second, second_sender) = dependency(ServiceStatus::Starting);
        let wait = wait_for_all_with_policy(
            vec![(1, first), (2, second)],
            ServiceStatus::Ready,
            Some(Duration::from_secs(5)),
            WaitPolicy::FailFast,
            CompletionPolicy::TargetReachedOnce,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        tokio::time::advance(Duration::from_secs(4)).await;
        first_sender.send_replace(ServiceStatus::Ready);
        second_sender.send_replace(ServiceStatus::Ready);
        assert!(wait.await.is_ok());
    }

    #[tokio::test]
    async fn closed_channel_preserves_error() {
        let (watcher, sender) = dependency(ServiceStatus::Starting);
        drop(sender);
        let error = wait_for_all_with_policy(
            vec![("closed", watcher)],
            ServiceStatus::Ready,
            None,
            WaitPolicy::FailFast,
            CompletionPolicy::TargetReachedOnce,
        )
        .await
        .unwrap_err();
        assert_eq!(
            *error.wait_error().unwrap(),
            WaitForError::ChannelClosed {
                initial_status: ServiceStatus::Starting,
                latest_status: ServiceStatus::Starting
            }
        );
    }
}
