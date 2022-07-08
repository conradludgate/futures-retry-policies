#![cfg(feature = "tokio")]
#![cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
//! Retry features for the [tokio runtime](https:://tokio.rs)

use std::{future::Future, time::Duration};
use tokio::time::{sleep, Sleep};

use crate::{RetryFuture, RetryPolicy};

/// Retry a future using the given [retry policy](`RetryPolicy`) and [tokio's sleep](`sleep`) method.
///
/// ```
/// use futures_retry_policies::{tokio::retry, RetryPolicy};
/// use std::{ops::ControlFlow, time::Duration};
///
/// pub struct Retries(usize);
/// impl RetryPolicy<Result<(), &'static str>> for Retries {
///     fn should_retry(&mut self, result: Result<(), &'static str>) -> ControlFlow<Result<(), &'static str>, Duration> {
///         if self.0 > 0 && result.is_err() {
///             self.0 -= 1;
///             // continue to retry on error
///             ControlFlow::Continue(Duration::from_millis(100))
///         } else {
///             // We've got a success, or we've exhauted our retries, so break
///             ControlFlow::Break(result)
///         }
///     }
/// }
///
/// async fn make_request() -> Result<(), &'static str>  {
///     // make a request
///     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
///     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err("fail") } else { Ok(()) }
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), &'static str> {
///     retry(Retries(3), make_request).await
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
/// use futures_retry_policies::{tokio::RetryFutureExt, RetryPolicy};
/// use std::{ops::ControlFlow, time::Duration};
///
/// pub struct Attempts(usize);
/// impl RetryPolicy<Result<(), &'static str>> for Attempts {
///     fn should_retry(&mut self, result: Result<(), &'static str>) -> ControlFlow<Result<(), &'static str>, Duration> {
///         self.0 -= 1;
///         if self.0 > 0 && result.is_err() {
///             // continue to retry on error
///             ControlFlow::Continue(Duration::from_millis(100))
///         } else {
///             // We've got a success, or we've exhauted our retries, so break
///             ControlFlow::Break(result)
///         }
///     }
/// }
///
/// async fn make_request() -> Result<(), &'static str>  {
///     // make a request
///     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
///     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err("fail") } else { Ok(()) }
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), &'static str> {
///     make_request.retry(Attempts(3)).await
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
