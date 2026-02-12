//! Node.js sidecar availability check.

use std::time::Duration;

use thiserror::Error;
use tokio::process::Command;
use tokio::time::timeout;

/// Maximum time to wait for `node --version` to respond.
const NODE_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Errors that can occur when checking Node.js availability.
#[derive(Debug, Error)]
pub enum SidecarError {
    #[error("Node.js not found on PATH")]
    NotFound,

    #[error("Node.js check timed out after {0:?}")]
    Timeout(Duration),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for sidecar operations.
pub type Result<T> = std::result::Result<T, SidecarError>;

/// Information about the Node.js sidecar runtime.
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Node.js version string (e.g. "v24.3.0").
    pub version: String,
    /// Whether Node.js is available and responding.
    pub available: bool,
}

/// Checks whether Node.js is available on PATH by running `node --version`.
pub async fn check_node_available() -> Result<NodeInfo> {
    let output = timeout(
        NODE_CHECK_TIMEOUT,
        Command::new("node").arg("--version").output(),
    )
    .await
    .map_err(|_| SidecarError::Timeout(NODE_CHECK_TIMEOUT))?
    .map_err(|_| SidecarError::NotFound)?;

    if !output.status.success() {
        return Err(SidecarError::NotFound);
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok(NodeInfo {
        version,
        available: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn check_node_returns_version() {
        let info = check_node_available()
            .await
            .expect("Node should be available via mise");
        assert!(info.available);
        assert!(
            info.version.starts_with('v'),
            "version should start with 'v': {}",
            info.version
        );
    }
}
