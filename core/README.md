# futures-retry-policies-core

A crate to help retry futures.

```rust
use futures_retry_policies_core::{retry, RetryPolicy};
use std::{ops::ControlFlow, time::Duration};

// 1. Create your retry policy

/// Retries a request n times
pub struct Retries(usize);

// 2. Define how your policy behaves

impl RetryPolicy<Result<(), &'static str>> for Retries {
    fn should_retry(&mut self, result: Result<(), &'static str>) -> ControlFlow<Result<(), &'static str>, Duration> {
        if self.0 > 0 && result.is_err() {
            self.0 -= 1;
            // continue to retry on error
            ControlFlow::Continue(Duration::from_millis(100))
        } else {
            // We've got a success, or we've exhausted our retries, so break
            ControlFlow::Break(result)
        }
    }
}

/// Makes a request, like a HTTP request or gRPC request which you want to retry
async fn make_request() -> Result<(), &'static str>  {
    // make a request
    # static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    # if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 { Err("fail") } else { Ok(()) }
}

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    // 3. Await the retry with your policy, a sleep function, and your async function.
    retry(Retries(3), tokio::time::sleep, make_request).await
}
```
