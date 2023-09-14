#![cfg_attr(docsrs, feature(doc_cfg))]
//! A crate to help retry futures.
//!
//! ```
//! use futures_retry_policies::{retry, RetryPolicy};
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
//!             // We've got a success, or we've exhauted our retries, so break
//!             ControlFlow::Break(result)
//!         }
//!     }
//! }
//!
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
//!
//! ## Tracing
//!
//! Add the `tracing` feature and you can use the `Traced` RetryPolicy to automatically
//! log your retries
//!
//! ## Tokio
//!
//! Add the `tokio` feature and you can use the convenience tokio retry methods to skip specifying
//! [`tokio::time::sleep`](::tokio::time::sleep).
//!
//! You can also use the [`tokio::RetryFutureExt`] trait to support calling `retry` directly
//! on async functions.
//!
//! ```
//! # use futures_retry_policies::{retry, RetryPolicy};
//! # use std::{ops::ControlFlow, time::Duration};
//! # pub struct Retries(usize);
//! # impl RetryPolicy<Result<(), &'static str>> for Retries {
//! #     fn should_retry(&mut self, result: Result<(), &'static str>) -> ControlFlow<Result<(), &'static str>, Duration> {
//! #         if self.0 > 0 && result.is_err() {
//! #             self.0 -= 1;
//! #             ControlFlow::Continue(Duration::from_millis(100))
//! #         } else {
//! #             ControlFlow::Break(result)
//! #         }
//! #     }
//! # }
//! # async fn make_request() -> Result<(), &'static str>  {
//!     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
//!     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err("fail") } else { Ok(()) }
//! # }
//! use futures_retry_policies::tokio::RetryFutureExt;
//! #[tokio::main]
//! async fn main() -> Result<(), &'static str> {
//!     make_request.retry(Retries(3)).await
//! }
//! ```
//!
//! ## retry-policies
//!
//! This crate has first class support for the [`retry-policies crate`](::retry_policies)
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

pub mod futures_retry;
pub mod iter;
pub mod retry_policies;
pub mod sync;
pub mod tokio;
pub mod tracing;

pub use futures_retry_policies_core::{retry, RetryFuture, RetryPolicy};

/// A simpler form of [`RetryPolicy`] that returns whether
/// the value can be retried.
pub trait ShouldRetry {
    /// Whether the value should be re-attempted.
    /// Return true if a retry is permitted, return false if a retry is forbidden.
    /// `attempts` denotes how many prior attempts have been made (starts at 1).
    fn should_retry(&self, attempts: u32) -> bool;
}

impl<T, E: ShouldRetry> ShouldRetry for Result<T, E> {
    /// Result should retry if the error should retry.
    /// Should not retry if ok
    fn should_retry(&self, attempts: u32) -> bool {
        match self {
            Ok(_) => false,
            Err(e) => e.should_retry(attempts),
        }
    }
}

impl<T> ShouldRetry for Option<T> {
    /// Should retry if None
    fn should_retry(&self, _: u32) -> bool {
        self.is_none()
    }
}
