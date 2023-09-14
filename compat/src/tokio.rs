#![cfg(feature = "tokio")]
#![cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
//! Retry features for the [tokio runtime](https://tokio.rs)

use std::{future::Future, time::Duration};
use tokio::time::{sleep, Sleep};

use crate::RetryPolicy;

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
///             // We've got a success, or we've exhausted our retries, so break
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
) -> RetryFuture<Policy, Futures, Fut>
where
    Policy: RetryPolicy<Fut::Output>,
    Futures: FnMut() -> Fut,
    Fut: Future,
{
    super::retry(backoff, sleep, futures)
}

pub type RetryFuture<Policy, Futures, Fut> =
    crate::RetryFuture<Policy, fn(Duration) -> Sleep, Sleep, Futures, Fut>;

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
///             // We've got a success, or we've exhausted our retries, so break
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
    fn retry<Policy>(self, policy: Policy) -> RetryFuture<Policy, Self, Fut>
    where
        Policy: RetryPolicy<Fut::Output>,
        Self: FnMut() -> Fut + Sized,
    {
        retry(policy, self)
    }
}

impl<Futures, Fut> RetryFutureExt<Fut> for Futures
where
    Futures: FnMut() -> Fut,
    Fut: Future,
{
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_retry_policies_core::RetryPolicy;
    use retry_policies::policies::ExponentialBackoff;

    use crate::{retry_policies::RetryPolicies, tokio::RetryFutureExt, ShouldRetry};

    struct Error(u32);
    impl ShouldRetry for Error {
        fn should_retry(&self, attempts: u32) -> bool {
            attempts <= self.0
        }
    }

    /// exponential backoff with no jitter, max 3 attempts.
    /// 1s * 2^0, 2^1, 2^2
    fn policy<R: ShouldRetry>() -> impl RetryPolicy<R> {
        let backoff = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(1), Duration::from_secs(60))
            .jitter(retry_policies::Jitter::None)
            .build_with_max_retries(3);
        RetryPolicies::new(backoff)
    }

    #[tokio::test(start_paused = true)]
    async fn retry_full() {
        async fn req() -> Result<(), Error> {
            // always retry
            Err(Error(u32::MAX))
        }

        let start = tokio::time::Instant::now();
        req.retry(policy()).await.unwrap_err();
        assert_eq!(start.elapsed(), Duration::from_secs(1 + 2 + 4));
    }

    #[tokio::test(start_paused = true)]
    async fn retry_none() {
        async fn req() -> Result<(), Error> {
            // never retry
            Err(Error(0))
        }

        let start = tokio::time::Instant::now();
        req.retry(policy()).await.unwrap_err();
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_twice() {
        async fn req() -> Result<(), Error> {
            // retry twice
            Err(Error(2))
        }

        let start = tokio::time::Instant::now();
        req.retry(policy()).await.unwrap_err();
        assert_eq!(start.elapsed(), Duration::from_secs(1 + 2));
    }
}
