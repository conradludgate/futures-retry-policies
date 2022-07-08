use std::{future::Future, time::Duration};
use tokio::time::{sleep, Sleep};

use crate::{RetryFuture, RetryPolicy};

/// Retry a future using the given [backoff policy](`RetryPolicy`) and [tokio's sleep](`sleep`) method.
///
/// ```
/// use futures_retry_policies::{tokio::retry, ShouldRetry};
/// use retry_policies::policies::ExponentialBackoff;
///
/// # #[derive(Debug)]
/// enum Error { Retry, DoNotRetry }
/// impl ShouldRetry for Error {
///     fn should_retry(&self) -> bool { matches!(self, Error::Retry) }
/// }
/// async fn make_request() -> Result<(), Error>  {
///     // make a request
///     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
///     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err(Error::Retry) } else { Ok(()) }
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let backoff = ExponentialBackoff::builder().build_with_max_retries(3);
///     retry(backoff, make_request).await
/// }
/// ```
pub fn retry<Policy, Futures, Fut>(
    backoff: Policy,
    futures: Futures,
) -> RetryFuture<Policy, fn(Duration) -> Sleep, Sleep, Futures, Fut>
where
    Policy: RetryPolicy<Fut::Output>,
    Futures: Fn() -> Fut,
    Fut: Future,
{
    super::retry(backoff, sleep, futures)
}

/// Easy helper trait to retry futures
///
/// ```
/// use futures_retry_policies::{tokio::RetryFutureExt, ShouldRetry};
/// use retry_policies::policies::ExponentialBackoff;
///
/// # #[derive(Debug)]
/// enum Error { Retry, DoNotRetry }
/// impl ShouldRetry for Error {
///     fn should_retry(&self) -> bool { matches!(self, Error::Retry) }
/// }
/// async fn make_request() -> Result<(), Error>  {
///     // make a request
///     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
///     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err(Error::Retry) } else { Ok(()) }
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let backoff = ExponentialBackoff::builder().build_with_max_retries(3);
///     make_request.retry(backoff).await
/// }
/// ```
pub trait RetryFutureExt<Fut>
where
    Fut: Future,
{
    fn retry<Policy>(
        self,
        policy: Policy,
    ) -> RetryFuture<Policy, fn(Duration) -> Sleep, Sleep, Self, Fut>
    where
        Policy: RetryPolicy<Fut::Output>,
        Self: Fn() -> Fut + Sized,
    {
        retry(policy, self)
    }
}

impl<Futures, Fut> RetryFutureExt<Fut> for Futures
where
    Futures: Fn() -> Fut,
    Fut: Future,
{
}
