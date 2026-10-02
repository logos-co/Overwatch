use std::fmt::{Display, Formatter};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ServiceStatus {
    /// The `Service` is in the process of being started.
    Starting,
    /// The `Service` is ready to operate.
    ///
    /// This is the responsibility of the `Service` to send this message.
    /// Because of this, it might not be sent.
    Ready,
    /// The `Service` has been stopped.
    ///
    /// It can be restarted by sending the appropriate
    /// [`LifecycleMessage`](crate::services::lifecycle::LifecycleMessage).
    Stopped,
    /// The `Service` has been stopped because its
    /// [`run`](crate::services::ServiceCore::run) panicked.
    ///
    /// Like [`Self::Stopped`], it can be restarted by sending the appropriate
    /// [`LifecycleMessage`](crate::services::lifecycle::LifecycleMessage).
    Failed,
}

impl Display for ServiceStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use ServiceStatus::{Failed, Ready, Starting, Stopped};
        let service_status = match self {
            Starting => "ServiceStatus::Starting",
            Ready => "ServiceStatus::Ready",
            Stopped => "ServiceStatus::Stopped",
            Failed => "ServiceStatus::Failed",
        };
        write!(f, "{service_status}")
    }
}
