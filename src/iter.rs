//! Convenience types for using iterators as policies.
//!
//! This allows for good interop with the [retry crate](::retry)

use std::{ops::ControlFlow, time::Duration};

use crate::{RetryPolicy, ShouldRetry};

/// Iter type for specifying an iterator as a retry policy
///
/// ```
/// use ::retry::delay::{Fibonacci, jitter};
/// use futures_retry_policies::{tokio::RetryFutureExt, iter::Iter};
///
/// async fn make_request() -> Option<()> {
///     // make a request
///     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
///     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { None } else { Some(()) }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     make_request
///         .retry(Iter::new(Fibonacci::from_millis(10).map(jitter).take(3)))
///         .await
///         .unwrap()
/// }
/// ```
pub struct Iter<I> {
    iter: I,
    amount: u32,
}

impl<I: Iterator> Iter<I> {
    pub fn new(iter: impl IntoIterator<IntoIter = I>) -> Self {
        Self {
            iter: iter.into_iter(),
            amount: 0,
        }
    }
}

impl<R, I> RetryPolicy<R> for Iter<I>
where
    R: ShouldRetry,
    I: Iterator<Item = Duration>,
{
    fn should_retry(&mut self, result: R) -> std::ops::ControlFlow<R, Duration> {
        self.amount += 1;
        match self.iter.next() {
            Some(duration) if result.should_retry(self.amount) => ControlFlow::Continue(duration),
            _ => ControlFlow::Break(result),
        }
    }
}

#[cfg(feature = "retry-crate")]
impl<T, E> ShouldRetry for retry::OperationResult<T, E> {
    fn should_retry(&self, _: u32) -> bool {
        matches!(self, retry::OperationResult::Retry(_))
    }
}
