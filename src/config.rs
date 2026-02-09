use std::path::PathBuf;
use std::time::Duration;

/// Default timeout for tool operations (30 seconds).
const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 30;

/// Server configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub cache_root: PathBuf,
    pub tool_timeout: Duration,
}

impl ServerConfig {
    /// Load configuration from environment.
    ///
    /// - `CONTEXT_CACHE_ROOT` (required) — root directory for caches
    /// - `CONTEXT_TOOL_TIMEOUT_SECS` (optional, default 30) — max seconds per tool call
    pub fn from_env() -> Result<Self, String> {
        let cache_root = std::env::var("CONTEXT_CACHE_ROOT")
            .map(PathBuf::from)
            .map_err(|_| "CONTEXT_CACHE_ROOT environment variable is not set".to_string())?;

        let tool_timeout_secs = match std::env::var("CONTEXT_TOOL_TIMEOUT_SECS") {
            Ok(val) => val
                .parse::<u64>()
                .map_err(|_| "CONTEXT_TOOL_TIMEOUT_SECS must be a positive integer".to_string())?,
            Err(_) => DEFAULT_TOOL_TIMEOUT_SECS,
        };

        Ok(Self {
            cache_root,
            tool_timeout: Duration::from_secs(tool_timeout_secs),
        })
    }
}
