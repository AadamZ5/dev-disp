use std::future::Future;

use futures::FutureExt;

/// An async runtime.
pub trait Runtime: Clone {
    type Error;

    /// Spawn a future that may be sent to another thread.
    fn spawn<F>(
        &mut self,
        future: F,
    ) -> impl Future<Output = Result<F::Output, Self::Error>> + Send + 'static
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static;

    /// Spawn a future that will not be sent to another thread.
    // fn spawn_local<F>(
    //     &mut self,
    //     future: F,
    // ) -> impl Future<Output = Result<F::Output, Self::Error>> + 'static
    // where
    //     F: std::future::Future + 'static,
    //     F::Output: 'static;

    /// Spawn a blocking task that will run on a dedicated thread pool.
    fn spawn_blocking<F>(
        &mut self,
        work: F,
    ) -> impl Future<Output = Result<F::Output, Self::Error>> + Send + 'static
    where
        F: FnOnce() + Send + 'static,
        F::Output: Send + 'static;

    /// Sleep for the specified duration asynchronously.
    fn sleep(
        &mut self,
        duration: std::time::Duration,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static;

    /// Sleep until the specified instant asynchronously.
    fn sleep_until(
        &mut self,
        instant: std::time::Instant,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static;
}

#[derive(Clone, Debug)]
pub struct TokioRuntime;

impl Runtime for TokioRuntime {
    type Error = tokio::task::JoinError;

    fn spawn<F>(
        &mut self,
        future: F,
    ) -> impl Future<Output = Result<F::Output, Self::Error>> + Send + 'static
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        tokio::spawn(future)
    }

    fn spawn_blocking<F>(
        &mut self,
        work: F,
    ) -> impl Future<Output = Result<F::Output, Self::Error>> + Send + 'static
    where
        F: FnOnce() + Send + 'static,
        F::Output: Send + 'static,
    {
        tokio::task::spawn_blocking(work)
    }

    fn sleep(
        &mut self,
        duration: std::time::Duration,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        tokio::time::sleep(duration).map(|_| Ok(()))
    }

    fn sleep_until(
        &mut self,
        instant: std::time::Instant,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        tokio::time::sleep_until(instant.into()).map(|_| Ok(()))
    }
}
