use crate::utils::finished_signal;

/// Message type for
/// [`LifecycleHandle`](crate::services::lifecycle::handle::LifecycleHandle).
#[derive(Debug)]
pub enum LifecycleMessage {
    /// Starts the `Service`.
    ///
    /// If the `Service` has been stopped with [`LifecycleMessage::Stop`], it
    /// will be restarted.
    ///
    /// # Arguments
    ///
    /// - [`finished_signal::Sender`]: A [`finished_signal::Signal`] will be
    ///   sent through the associated channel upon completion of the task.
    Start(finished_signal::Sender),

    /// Stops the `Service`.
    ///
    /// Inner `Service` operations are not guaranteed to be completed.
    /// Despite that, `Service`s stopped this way can be restarted (from a
    /// previously saved point or from the default initial state) by sending
    /// a [`LifecycleMessage::Start`].
    ///
    /// # Arguments
    ///
    /// - [`finished_signal::Sender`]: A [`finished_signal::Signal`] will be
    ///   sent through the associated channel upon completion of the task.
    Stop(finished_signal::Sender),
}
