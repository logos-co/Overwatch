/// Abstraction over the supported runtimes for
/// [`Overwatch`](crate::overwatch::Overwatch).
pub enum OverwatchRuntime {
    TokioRuntime(tokio::runtime::Runtime),
    TokioHandle(tokio::runtime::Handle),
}

impl OverwatchRuntime {
    /// Returns a reference to the runtime's [`Handle`](tokio::runtime::Handle).
    pub fn handle(&self) -> &tokio::runtime::Handle {
        match self {
            Self::TokioRuntime(runtime) => runtime.handle(),
            Self::TokioHandle(handle) => handle,
        }
    }

    /// Shuts down the runtime in the background.
    pub fn shutdown_background(self) {
        match self {
            Self::TokioRuntime(runtime) => runtime.shutdown_background(),
            Self::TokioHandle(_) => {} // handle doesn't own the runtime, nothing to do
        }
    }
}
