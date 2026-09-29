//! One shared tokio runtime for all background work (HTTP and keyring). Qt stays on
//! its own thread; results come back with `qt_thread().queue(...)`.

use std::future::Future;
use std::sync::LazyLock;

use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

/// The process-wide runtime, built on first use.
pub static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("crumb-desktop")
        .build()
        .expect("failed to build the tokio runtime")
});

/// Spawns a future on the shared runtime.
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    RUNTIME.spawn(future)
}
