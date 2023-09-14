#![cfg(feature = "tracing")]
#![cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
//! Retry features for [`tracing`] support

use std::{fmt::Debug, ops::ControlFlow, time::Duration};

use crate::RetryPolicy;

/// A [`RetryPolicy`] that logs a warning if a retry is being attempted
pub struct Traced<P>(pub P);

impl<P, R> RetryPolicy<R> for Traced<P>
where
    P: RetryPolicy<R>,
    R: Debug,
{
    fn should_retry(&mut self, result: R) -> ControlFlow<R, Duration> {
        // get debug output of result eagerly so we can log it if we need to retry
        let res = format!("{result:?}");

        let duration = self.0.should_retry(result)?;
        tracing::warn!(?duration, res, "waiting to retrying request");
        ControlFlow::Continue(duration)
    }
}
