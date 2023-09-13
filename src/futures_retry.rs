#![cfg(feature = "futures-retry")]
#![cfg_attr(docsrs, doc(cfg(feature = "futures-retry")))]
//! Retry interop with [`futures-retry`](futures_retry)

use std::{ops::ControlFlow, time::Duration};

use futures_retry::ErrorHandler;

use crate::RetryPolicy;

/// [`RetryPolicy`] with [`futures_retry`] support.
///
/// ```
/// use futures_retry_policies::{tokio::RetryFutureExt, futures_retry::FuturesRetryPolicy};
/// use futures_retry::RetryPolicy;
///
/// async fn make_request() -> Result<(), &'static str>  {
///     // make a request
///     # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
///     # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err("retry") } else { Ok(()) }
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), &'static str> {
///     make_request.retry(FuturesRetryPolicy::new(|e| {
///         if e == "retry" {
///             RetryPolicy::Repeat
///         } else {
///             RetryPolicy::ForwardError(e)
///         }
///     })).await
/// }
/// ```
pub struct FuturesRetryPolicy<H> {
    handle: H,
    amount: usize,
}

impl<H> FuturesRetryPolicy<H> {
    pub fn new(handle: H) -> Self {
        Self { handle, amount: 0 }
    }
}

impl<T, E, H> RetryPolicy<Result<T, E>> for FuturesRetryPolicy<H>
where
    H: ErrorHandler<E, OutError = E>,
{
    fn should_retry(&mut self, result: Result<T, E>) -> ControlFlow<Result<T, E>, Duration> {
        self.amount += 1;
        match result {
            Ok(result) => {
                self.handle.ok(self.amount);
                ControlFlow::Break(Ok(result))
            }
            Err(err) => match self.handle.handle(self.amount, err) {
                futures_retry::RetryPolicy::Repeat => ControlFlow::Continue(Duration::ZERO),
                futures_retry::RetryPolicy::WaitRetry(dur) => ControlFlow::Continue(dur),
                futures_retry::RetryPolicy::ForwardError(err) => ControlFlow::Break(Err(err)),
            },
        }
    }
}
