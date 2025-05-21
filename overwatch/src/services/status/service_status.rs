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
    /// [`LifecycleMessage`](crate::services::life_cycle::LifecycleMessage).
    Stopped,
}
