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

/// Polyfills for the [`retry_policies`] crate
#[cfg(feature = "retry-policies")]
#[cfg_attr(docsrs, doc(cfg(feature = "retry-policies")))]
pub mod retry_policies;

/// Retry features for the [tokio runtime](https:://tokio.rs)
#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
pub mod tokio;

/// Retry features for [`tracing`] support
#[cfg(feature = "tracing")]
#[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
pub mod tracing;

use std::{
    future::Future,
    ops::ControlFlow,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use pin_project::pin_project;
// use retry_policies::{RetryDecision, RetryPolicy};

/// Policy to decide whether a result should be retried
pub trait RetryPolicy<Res> {
    /// Determine if the request should be retried under the given policy.
    /// Return `Break(result)` if we are done retrying
    /// Else, return `Continue(duration)` to sleep for that duration.
    fn should_retry(&mut self, result: Res) -> ControlFlow<Res, Duration>;
}

impl<P: RetryPolicy<R>, R> RetryPolicy<R> for &mut P {
    fn should_retry(&mut self, result: R) -> ControlFlow<R, Duration> {
        P::should_retry(self, result)
    }
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
    Policy: RetryPolicy<Fut::Output>,
    Sleeper: Fn(Duration) -> Sleep,
    Sleep: Future<Output = ()>,
    Futures: Fn() -> Fut,
    Fut: Future,
{
    RetryFuture {
        backoff,
        sleeper,
        futures,
        state: RetryState::Idle,
    }
}

/// [`Future`] returned by [`retry`]
#[pin_project]
pub struct RetryFuture<Policy, Sleeper, Sleep, Futures, Fut> {
    backoff: Policy,
    sleeper: Sleeper,
    futures: Futures,
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
    Policy: RetryPolicy<Fut::Output>,
    Sleeper: Fn(Duration) -> Sleep,
    Sleep: Future<Output = ()>,
    Futures: Fn() -> Fut,
    Fut: Future,
{
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        // retry state machine
        // 1. State goes from idle to attempting
        // 2. Once the attempt is ready, it checks if a retry is necessary.
        // 3. If a retry is necessary:
        //   i.   We transition to sleeping.
        //   ii.  And then transition back to idle.
        //   iii. And then loop back to 1
        loop {
            match this.state.as_mut().project() {
                RetryStateProj::Idle => this.state.set(RetryState::Attempts((this.futures)())),
                RetryStateProj::Attempts(fut) => match fut.poll(cx) {
                    Poll::Ready(res) => match this.backoff.should_retry(res) {
                        ControlFlow::Continue(sleep) => {
                            this.state.set(RetryState::Sleeping((this.sleeper)(sleep)));
                        }
                        ControlFlow::Break(res) => return Poll::Ready(res),
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
