pub mod backend;

#[cfg(target_arch = "wasm32")]
pub mod playwright_harness;

#[cfg(target_arch = "wasm32")]
pub use backend::BrowserBackend;
#[cfg(target_arch = "wasm32")]
pub use playwright_harness::run_surf_provider_browser_integration_tests;
pub use surf_http_backend_config::HttpBackendConfig;
