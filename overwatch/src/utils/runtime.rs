// internal
use crate::overwatch::OVERWATCH_THREAD_NAME;

#[must_use]
/// Build the default Tokio runtime.
///
/// # Panics
///
/// If the default runtime with the default config cannot be created.
pub fn default_multithread_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name(OVERWATCH_THREAD_NAME)
        .build()
        .expect("Async runtime to build properly")
}
