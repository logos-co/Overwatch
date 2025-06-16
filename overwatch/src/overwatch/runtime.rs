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
}
