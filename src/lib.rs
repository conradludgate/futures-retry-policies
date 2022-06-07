//! Retry a future using the given [backoff policy](`RetryPolicy`) and sleep function.
//!
//! ```
//! use futures_retry_policies::{tokio::retry, ShouldRetry};
//! use retry_policies::policies::ExponentialBackoff;
//!
//! # #[derive(Debug)]
//! enum Error { Retry, DoNotRetry }
//! impl ShouldRetry for Error {
//!     fn should_retry(&self) -> bool { matches!(self, Error::Retry) }
//! }
//! async fn make_request() -> Result<(), Error>  {
//!     // make a request
//!     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
//!     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err(Error::Retry) } else { Ok(()) }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let backoff = ExponentialBackoff::builder().build_with_max_retries(3);
//!     retry(backoff, make_request).await
//! }
//! ```
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use chrono::Utc;
use pin_project::pin_project;
use retry_policies::{RetryDecision, RetryPolicy};

pub trait ShouldRetry {
    fn should_retry(&self) -> bool;
}

impl<T, E: ShouldRetry> ShouldRetry for Result<T, E> {
    /// Returns true if the error should retry. Returns false if ok.
    fn should_retry(&self) -> bool {
        match self {
            Ok(_) => false,
            Err(e) => e.should_retry(),
        }
    }
}

impl<T> ShouldRetry for Option<T> {
    /// Returns true if the value is None.
    fn should_retry(&self) -> bool {
        self.is_none()
    }
}

/// Retry features for the [tokio runtime](https:://tokio.rs)
#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
pub mod tokio {
    use retry_policies::RetryPolicy;
    use std::{future::Future, time::Duration};
    use tokio::time::{sleep, Sleep};

    use crate::{RetryFuture, ShouldRetry};

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
        Policy: RetryPolicy,
        Futures: Fn() -> Fut,
        Fut: Future,
        Fut::Output: ShouldRetry,
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
        Fut::Output: ShouldRetry,
    {
        fn retry<Policy>(self, backoff: Policy) -> RetryFuture<Policy, fn(Duration) -> Sleep, Sleep, Self, Fut>
        where
            Policy: RetryPolicy,
            Self: Fn() -> Fut + Sized,
        {
            retry(backoff, self)
        }
    }

    impl<Futures, Fut> RetryFutureExt<Fut> for Futures
    where
        Futures: Fn() -> Fut,
        Fut: Future,
        Fut::Output: ShouldRetry,
    {}
}

/// Retry a future using the given [backoff policy](`RetryPolicy`) and sleep function.
///
/// ```
/// use futures_retry_policies::{retry, ShouldRetry};
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
///     retry(backoff, tokio::time::sleep, make_request).await
/// }
/// ```
pub fn retry<Policy, Sleeper, Sleep, Futures, Fut>(
    backoff: Policy,
    sleeper: Sleeper,
    futures: Futures,
) -> RetryFuture<Policy, Sleeper, Sleep, Futures, Fut>
where
    Policy: RetryPolicy,
    Sleeper: Fn(Duration) -> Sleep,
    Sleep: Future<Output = ()>,
    Futures: Fn() -> Fut,
    Fut: Future,
    Fut::Output: ShouldRetry,
{
    RetryFuture {
        backoff,
        sleeper,
        futures,
        retries: 0,
        state: RetryState::Idle,
    }
}

/// [`Future`] returned by [`retry`]
#[pin_project]
pub struct RetryFuture<Policy, Sleeper, Sleep, Futures, Fut> {
    backoff: Policy,
    sleeper: Sleeper,
    futures: Futures,
    retries: u32,
    #[pin]
    state: RetryState<Sleep, Fut>,
}

#[pin_project(project = RetryStateProj)]
enum RetryState<Sleep, Fut> {
    Idle,
    Sleeping(#[pin] Sleep),
    Attempts(#[pin] Fut),
}

impl<Policy, Sleeper, Sleep, Futures, Fut> Future
    for RetryFuture<Policy, Sleeper, Sleep, Futures, Fut>
where
    Policy: RetryPolicy,
    Sleeper: Fn(Duration) -> Sleep,
    Sleep: Future<Output = ()>,
    Futures: Fn() -> Fut,
    Fut: Future,
    Fut::Output: ShouldRetry,
{
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        // retry state machine
        // 1. State goes from idle to attempting
        // 2. Once the attempt is ready, it checks if a retry is necessary.
        // 3. If a retry is necessary:
        //   i.   It increments the retries count and then transitions to sleeping.
        //   ii.  It then transitions back to idle.
        //   iii. And then loops back to 1
        loop {
            match this.state.as_mut().project() {
                RetryStateProj::Idle => this.state.set(RetryState::Attempts((this.futures)())),
                RetryStateProj::Attempts(fut) => match fut.poll(cx) {
                    Poll::Ready(res) => match this.backoff.should_retry(*this.retries) {
                        RetryDecision::Retry { execute_after } if res.should_retry() => {
                            *this.retries += 1;
                            let dur = (execute_after - Utc::now()).to_std().unwrap_or_default();
                            this.state.set(RetryState::Sleeping((this.sleeper)(dur)));
                        }
                        _ => return Poll::Ready(res),
                    },
                    Poll::Pending => return Poll::Pending,
                },
                RetryStateProj::Sleeping(sleep) => match sleep.poll(cx) {
                    Poll::Ready(()) => this.state.set(RetryState::Idle),
                    Poll::Pending => return Poll::Pending,
                },
            }
        }
    }
}

/// A [`ShouldRetry`] wrapper that always returns true
#[derive(Debug)]
pub struct AlwaysRetry<T>(pub T);
impl<T> ShouldRetry for AlwaysRetry<T> {
    fn should_retry(&self) -> bool {
        true
    }
}

/// A [`ShouldRetry`] wrapper that always returns false
#[derive(Debug)]
pub struct NeverRetry<T>(pub T);
impl<T> ShouldRetry for NeverRetry<T> {
    fn should_retry(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::{AlwaysRetry, NeverRetry};

    use super::retry;
    use retry_policies::policies::ExponentialBackoff;
    use tokio::task::yield_now;

    #[tokio::test]
    async fn retries() {
        let backoff = ExponentialBackoff::builder().build_with_max_retries(3);

        let timeouts = Mutex::new(vec![]);
        let sleep = |dur| {
            timeouts.lock().unwrap().push(dur);
            yield_now()
        };
        let _: AlwaysRetry<()> = retry(backoff, sleep, || async { AlwaysRetry(()) }).await;

        assert_eq!(timeouts.lock().unwrap().len(), 3); // 3 total retry attempts
    }

    #[tokio::test]
    async fn eventually_succeed() {
        let backoff = ExponentialBackoff::builder().build_with_max_retries(3);

        let timeouts = Mutex::new(vec![]);
        let sleep = |dur| {
            timeouts.lock().unwrap().push(dur);
            yield_now()
        };
        retry(backoff, sleep, || async {
            if timeouts.lock().unwrap().len() < 2 {
                Err(AlwaysRetry(()))
            } else {
                Ok(())
            }
        })
        .await
        .unwrap();

        assert_eq!(timeouts.lock().unwrap().len(), 2); // succeeds after 2 retries
    }

    #[tokio::test]
    async fn immediately_succeed() {
        let backoff = ExponentialBackoff::builder().build_with_max_retries(3);

        let timeouts = Mutex::new(vec![]);
        let sleep = |dur| {
            timeouts.lock().unwrap().push(dur);
            yield_now()
        };
        retry(backoff, sleep, || async { NeverRetry(()) }).await;

        assert!(timeouts.lock().unwrap().is_empty()); // no retries
    }
}
