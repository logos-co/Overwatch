use crate::overwatch::OVERWATCH_THREAD_NAME;

pub fn default_multithread_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name(OVERWATCH_THREAD_NAME)
        .build()
        .expect("Async runtime to build properly")
}
