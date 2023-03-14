#![cfg(feature = "retry-policies")]
#![cfg_attr(docsrs, doc(cfg(feature = "retry-policies")))]
//! Polyfills for the [`retry_policies`] crate
//! Retry a future using the given [backoff policy](`RetryPolicy`) and sleep function.
//!
//! ```
//! use futures_retry_policies::{retry, retry_policies::{ShouldRetry, RetryPolicies}};
//! use retry_policies::policies::ExponentialBackoff;
//!
//! # #[derive(Debug)]
//! enum Error { Retry, DoNotRetry }
//! impl ShouldRetry for Error {
//!     fn should_retry(&self, _: u32) -> bool { matches!(self, Error::Retry) }
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
//!     let policy = RetryPolicies::new(backoff);
//!     retry(policy, tokio::time::sleep, make_request).await
//! }
//! ```
use std::{mem, ops::ControlFlow, time::Duration};

use chrono::Utc;
use retry_policies::RetryDecision;

use crate::RetryPolicy;

// exported for backwards compatability
pub use super::ShouldRetry;

pub struct RetryPolicies<P> {
    policy: P,
    amount: u32,
}

impl<P> RetryPolicies<P> {
    pub fn new(policy: P) -> Self {
        Self { policy, amount: 0 }
    }
}

impl<P, R> RetryPolicy<R> for RetryPolicies<P>
where
    P: retry_policies::RetryPolicy,
    R: ShouldRetry,
{
    fn should_retry(&mut self, result: R) -> ControlFlow<R, Duration> {
        let attempts = self.amount + 1;
        let n_past_retries = mem::replace(&mut self.amount, attempts);
        match self.policy.should_retry(n_past_retries) {
            RetryDecision::Retry { execute_after } if result.should_retry(attempts) => {
                ControlFlow::Continue((execute_after - Utc::now()).to_std().unwrap_or_default())
            }
            _ => ControlFlow::Break(result),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{RetryPolicies, ShouldRetry};

    use crate::retry;
    use retry_policies::policies::ExponentialBackoff;
    use tokio::task::yield_now;

    /// A [`ShouldRetry`] wrapper that always returns true
    #[derive(Debug)]
    pub struct AlwaysRetry;
    impl ShouldRetry for AlwaysRetry {
        fn should_retry(&self, _: u32) -> bool {
            true
        }
    }

    /// A [`ShouldRetry`] wrapper that always returns false
    #[derive(Debug)]
    pub struct NeverRetry;
    impl ShouldRetry for NeverRetry {
        fn should_retry(&self, _: u32) -> bool {
            false
        }
    }

    /// A [`ShouldRetry`] wrapper that always returns false
    #[derive(Debug)]
    pub struct RetryUntil(u32);
    impl ShouldRetry for RetryUntil {
        fn should_retry(&self, attempts: u32) -> bool {
            attempts < self.0
        }
    }

    async fn sleep(_: Duration) {
        yield_now().await
    }

    #[tokio::test]
    async fn retries() {
        let backoff = ExponentialBackoff::builder().build_with_max_retries(3);

        let mut policy = RetryPolicies::new(backoff);
        let _: AlwaysRetry = retry(&mut policy, sleep, || async { AlwaysRetry }).await;

        assert_eq!(policy.amount, 4); // 4 total attempts
    }

    #[tokio::test]
    async fn eventually_succeed() {
        let backoff = ExponentialBackoff::builder().build_with_max_retries(3);

        let mut policy = RetryPolicies::new(backoff);
        retry(&mut policy, sleep, || async { RetryUntil(2) }).await;

        assert_eq!(policy.amount, 2); // succeeds after 2 attempts
    }

    #[tokio::test]
    async fn immediately_succeed() {
        let backoff = ExponentialBackoff::builder().build_with_max_retries(3);

        let mut policy = RetryPolicies::new(backoff);
        retry(&mut policy, sleep, || async { NeverRetry }).await;

        assert_eq!(policy.amount, 1); // only 1 attempt
    }

    #[tokio::test]
    async fn immediately_succeed_fn_mut() {
        let backoff = ExponentialBackoff::builder().build_with_max_retries(3);

        let mut policy = RetryPolicies::new(backoff);
        let mut counter = 0;

        retry(&mut policy, sleep, || {
            counter += 1;
            async move {
                if counter < 2 {
                    Err(AlwaysRetry)
                } else {
                    Ok(())
                }
            }
        })
        .await
        .unwrap();

        // 2 attempts
        assert_eq!(counter, 2);
        assert_eq!(policy.amount, 2);
    }
}
