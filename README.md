Retry a future using the given [backoff policy](https://docs.rs/retry-policies/0.1.1/retry_policies/trait.RetryPolicy.html) and sleep function.

```rust
use futures_retry_policies::{tokio::retry, ShouldRetry};
use retry_policies::policies::ExponentialBackoff;

# #[derive(Debug)]
enum Error { Retry, DoNotRetry }
impl ShouldRetry for Error {
    fn should_retry(&self) -> bool { matches!(self, Error::Retry) }
}
async fn make_request() -> Result<(), Error>  {
    // make a request
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let backoff = ExponentialBackoff::builder().build_with_max_retries(3);
    retry(backoff, make_request).await
}
```
