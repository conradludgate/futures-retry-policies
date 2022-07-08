//! Blocking retries
//!
//! While this crate is intended for use with futures, there's nothing that stops [`RetryPolicy`]
//! from working in a sync fashion.

use std::{ops::ControlFlow, thread};

use crate::RetryPolicy;

/// Blocking retry function
///
/// ```rust
/// use futures_retry_policies::{sync::retry, RetryPolicy};
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
/// fn make_request() -> Result<(), &'static str>  {
///     // make a request
///     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
///     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err("fail") } else { Ok(()) }
/// }
///
/// fn main() -> Result<(), &'static str> {
///     retry(Attempts(3), make_request)
/// }
/// ```
pub fn retry<Policy, F, Output>(mut policy: Policy, f: F) -> Output
where
    Policy: RetryPolicy<Output>,
    F: Fn() -> Output,
{
    loop {
        match policy.should_retry(f()) {
            ControlFlow::Continue(dur) => thread::sleep(dur),
            ControlFlow::Break(result) => break result,
        }
    }
}

/// Easy helper trait to retry functions
///
/// ```
/// use futures_retry_policies::{sync::RetryFnExt, RetryPolicy};
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
/// fn make_request() -> Result<(), &'static str>  {
///     // make a request
///     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
///     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err("fail") } else { Ok(()) }
/// }
///
/// fn main() -> Result<(), &'static str> {
///     make_request.retry(Attempts(3))
/// }
/// ```
pub trait RetryFnExt<Output> {
    fn retry<Policy>(self, policy: Policy) -> Output
    where
        Policy: RetryPolicy<Output>,
        Self: Fn() -> Output + Sized,
    {
        retry(policy, self)
    }
}

impl<Futures, Output> RetryFnExt<Output> for Futures where Futures: Fn() -> Output {}
