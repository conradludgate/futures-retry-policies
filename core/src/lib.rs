//! A crate to help retry futures.
//!
//! ```
//! use futures_retry_policies_core::{retry, RetryPolicy};
//! use std::{ops::ControlFlow, time::Duration};
//!
//! // 1. Create your retry policy
//!
//! /// Retries a request n times
//! pub struct Retries(usize);
//!
//! // 2. Define how your policy behaves
//!
//! impl RetryPolicy<Result<(), &'static str>> for Retries {
//!     fn should_retry(&mut self, result: Result<(), &'static str>) -> ControlFlow<Result<(), &'static str>, Duration> {
//!         if self.0 > 0 && result.is_err() {
//!             self.0 -= 1;
//!             // continue to retry on error
//!             ControlFlow::Continue(Duration::from_millis(100))
//!         } else {
//!             // We've got a success, or we've exhausted our retries, so break
//!             ControlFlow::Break(result)
//!         }
//!     }
//! }
//!
//! /// Makes a request, like a HTTP request or gRPC request which you want to retry
//! async fn make_request() -> Result<(), &'static str>  {
//!     // make a request
//!     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
//!     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err("fail") } else { Ok(()) }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), &'static str> {
//!     // 3. Await the retry with your policy, a sleep function, and your async function.
//!     retry(Retries(3), tokio::time::sleep, make_request).await
//! }
//! ```
#![no_std]

use core::{
    future::Future,
    ops::ControlFlow,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};

use pin_project::pin_project;

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

/// Retry a future using the given [retry policy](`RetryPolicy`) and sleep function.
///
/// ```
/// use futures_retry_policies_core::{retry, RetryPolicy};
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
///     retry(Retries(3), tokio::time::sleep, make_request).await
/// }
/// ```
pub fn retry<Policy, Sleeper, Sleep, Futures, Fut>(
    policy: Policy,
    sleeper: Sleeper,
    futures: Futures,
) -> RetryFuture<Policy, Sleeper, Sleep, Futures, Fut>
where
    Policy: RetryPolicy<Fut::Output>,
    Sleeper: FnMut(Duration) -> Sleep,
    Sleep: Future<Output = ()>,
    Futures: FnMut() -> Fut,
    Fut: Future,
{
    RetryFuture {
        policy,
        sleeper,
        futures,
        state: RetryState::Idle,
    }
}

/// [`Future`] returned by [`retry`](crate::retry)
#[pin_project]
pub struct RetryFuture<Policy, Sleeper, Sleep, Futures, Fut> {
    policy: Policy,
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
    Sleeper: FnMut(Duration) -> Sleep,
    Sleep: Future<Output = ()>,
    Futures: FnMut() -> Fut,
    Fut: Future,
{
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Retry state machine:
        // 1. State goes from idle to attempting
        // 2. Once the attempt is ready, it checks if a retry is necessary.
        // 3. If a retry is necessary:
        //   i.   We transition to sleeping.
        //   ii.  And then transition back to idle.
        //   iii. And then loop back to 1
        //
        // This is the state-machine version of:
        // ```
        // loop {
        //     match self.policy.should_retry((self.futures)().await) {
        //         ControlFlow::Continue(dur) => (self.sleeper)(dur).await,
        //         ControlFlow::Break(result) => break result,
        //     }
        // }
        // ```
        let mut this = self.project();
        loop {
            match this.state.as_mut().project() {
                RetryStateProj::Idle => this.state.set(RetryState::Attempts((this.futures)())),
                RetryStateProj::Attempts(fut) => {
                    match this.policy.should_retry(ready!(fut.poll(cx))) {
                        ControlFlow::Continue(sleep) => {
                            this.state.set(RetryState::Sleeping((this.sleeper)(sleep)));
                        }
                        ControlFlow::Break(res) => return Poll::Ready(res),
                    }
                }
                RetryStateProj::Sleeping(sleep) => {
                    ready!(sleep.poll(cx));
                    this.state.set(RetryState::Idle)
                }
            }
        }
    }
}
