//! Isomorphic sleep provider for native and WASM platforms.

use std::time::Duration;

/// Provider for async sleep functionality.
///
/// This trait abstracts the sleep operation to work across different async runtimes.
/// Implementations are provided for both native (tokio) and WASM (gloo) platforms.
pub trait SleepProvider {
    fn sleep<'a>(
        &'a self,
        duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>>;
}

/// Tokio-based sleep provider for native platforms.
#[cfg(not(target_arch = "wasm32"))]
pub struct TokioSleep;

#[cfg(not(target_arch = "wasm32"))]
impl SleepProvider for TokioSleep {
    fn sleep<'a>(
        &'a self,
        duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
        Box::pin(async move {
            tokio::time::sleep(duration).await;
        })
    }
}

/// Gloo-based sleep provider for WASM platforms.
#[cfg(target_arch = "wasm32")]
pub struct WasmSleep;

#[cfg(target_arch = "wasm32")]
impl SleepProvider for WasmSleep {
    fn sleep<'a>(
        &'a self,
        duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
        Box::pin(async move {
            gloo::timers::future::TimeoutFuture::new(duration.as_millis() as u32).await;
        })
    }
}

// TODO: Uncomment when test dependencies are available
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use rstest::rstest;
//
//     #[rstest]
//     fn test_sleep_provider_trait_bounds() {
//         fn assert_send_sync<T: Send + Sync>() {}
//         #[cfg(not(target_arch = "wasm32"))]
//         assert_send_sync::<TokioSleep>();
//         #[cfg(target_arch = "wasm32")]
//         assert_send_sync::<WasmSleep>();
//     }
// }
