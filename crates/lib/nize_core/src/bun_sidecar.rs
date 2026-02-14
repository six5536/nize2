//! Bun sidecar availability check.

use std::time::Duration;

use thiserror::Error;
use tokio::process::Command;
use tokio::time::timeout;

/// Maximum time to wait for `bun --version` to respond.
const BUN_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Errors that can occur when checking Bun availability.
#[derive(Debug, Error)]
pub enum SidecarError {
    #[error("Bun not found on PATH")]
    NotFound,

    #[error("Bun check timed out after {0:?}")]
    Timeout(Duration),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for sidecar operations.
pub type Result<T> = std::result::Result<T, SidecarError>;

/// Information about the Bun sidecar runtime.
#[derive(Debug, Clone)]
pub struct BunInfo {
    /// Bun version string (e.g. "1.3.9").
    pub version: String,
    /// Whether Bun is available and responding.
    pub available: bool,
}

/// Checks whether Bun is available on PATH by running `bun --version`.
pub async fn check_bun_available() -> Result<BunInfo> {
    let output = timeout(
        BUN_CHECK_TIMEOUT,
        Command::new("bun").arg("--version").output(),
    )
    .await
    .map_err(|_| SidecarError::Timeout(BUN_CHECK_TIMEOUT))?
    .map_err(|_| SidecarError::NotFound)?;

    if !output.status.success() {
        return Err(SidecarError::NotFound);
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok(BunInfo {
        version,
        available: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn check_bun_returns_version() {
        let info = check_bun_available()
            .await
            .expect("Bun should be available via mise");
        assert!(info.available);
        assert!(
            !info.version.is_empty(),
            "version should not be empty: {}",
            info.version
        );
    }
}
