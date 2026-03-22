#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

const DEFAULT_URL: &str = "http://localhost:8899";
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const ENV_VALIDATOR_URL: &str = "SURF_TEST_VALIDATOR_URL";

#[derive(Debug, Clone)]
pub struct HttpBackendConfig {
    pub url: String,
    #[cfg(not(target_arch = "wasm32"))]
    pub timeout: Duration,
}

impl Default for HttpBackendConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_URL.to_string(),
            #[cfg(not(target_arch = "wasm32"))]
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }
}

impl HttpBackendConfig {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            ..Default::default()
        }
    }

    pub fn with_url(mut self, url: &str) -> Self {
        self.url = url.to_string();
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn from_env_or_default() -> Self {
        if let Ok(url) = std::env::var(ENV_VALIDATOR_URL) {
            Self::new(&url)
        } else {
            Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_config_new() {
        let config = HttpBackendConfig::new("http://example.com:8899");
        assert_eq!(config.url, "http://example.com:8899");
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[rstest]
    fn test_config_with_url() {
        let config = HttpBackendConfig::default().with_url("http://custom.validator:9999");
        assert_eq!(config.url, "http://custom.validator:9999");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[rstest]
    fn test_config_with_timeout() {
        let config = HttpBackendConfig::default().with_timeout(Duration::from_secs(60));
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[rstest]
    fn test_config_default() {
        let config = HttpBackendConfig::default();
        assert_eq!(config.url, "http://localhost:8899");
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[rstest]
    fn test_config_from_env() {
        std::env::set_var("SURF_TEST_VALIDATOR_URL", "http://env.validator:7777");
        let config = HttpBackendConfig::from_env_or_default();
        assert_eq!(config.url, "http://env.validator:7777");
        std::env::remove_var("SURF_TEST_VALIDATOR_URL");

        let config = HttpBackendConfig::from_env_or_default();
        assert_eq!(config.url, "http://localhost:8899");
    }
}
