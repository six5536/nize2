//! PostgreSQL database management.
//!
//! Provides `LocalDbManager` for lifecycle management of a local PostgreSQL
//! sidecar instance via process spawning (`initdb`, `pg_ctl`, `pg_isready`),
//! and `DbProvisioner` for database/extension provisioning against any
//! reachable PostgreSQL instance (local or remote).

use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

use sqlx::postgres::PgPool;
use thiserror::Error;
use tokio::process::Command;
use tokio::time::sleep;

/// Default database name for the Nize application.
const DEFAULT_DATABASE: &str = "nize";

/// Maximum time to wait for PostgreSQL to become ready.
const PG_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Poll interval when waiting for PostgreSQL readiness.
const PG_READY_POLL: Duration = Duration::from_millis(200);

/// Errors that can occur during database operations.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("PostgreSQL command failed: {0}")]
    Command(String),

    #[error("SQL error: {0}")]
    Sql(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Data directory not available")]
    NoDataDir,

    #[error("pg_config not found on PATH")]
    PgConfigNotFound,

    #[error("PostgreSQL not ready after {0:?}")]
    ReadyTimeout(Duration),
}

/// Result type for database operations.
pub type Result<T> = std::result::Result<T, DbError>;

/// Configuration for locating and running a PostgreSQL instance.
#[derive(Debug, Clone)]
pub struct PgConfig {
    /// Path to the PG bin directory (containing initdb, pg_ctl, pg_isready, etc.)
    pub bin_dir: PathBuf,
    /// Path to the PGDATA directory.
    pub data_dir: PathBuf,
    /// Port to listen on. Set to 0 to auto-assign a free ephemeral port.
    pub port: u16,
    /// Database name.
    pub database_name: String,
    /// Whether the data directory is temporary (cleaned up on drop).
    pub temporary: bool,
}

impl PgConfig {
    /// Discover PG binaries via `pg_config --bindir` on PATH.
    pub async fn from_env(data_dir: PathBuf, database_name: &str) -> Result<Self> {
        let output = Command::new("pg_config")
            .arg("--bindir")
            .output()
            .await
            .map_err(|_| DbError::PgConfigNotFound)?;

        if !output.status.success() {
            return Err(DbError::PgConfigNotFound);
        }

        let bin_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(Self {
            bin_dir: PathBuf::from(bin_dir),
            data_dir,
            port: 0,
            database_name: database_name.to_string(),
            temporary: false,
        })
    }

    /// Create a config with an explicit bin directory (for bundled sidecar).
    pub fn with_bin_dir(bin_dir: PathBuf, data_dir: PathBuf, database_name: &str) -> Self {
        Self {
            bin_dir,
            data_dir,
            port: 0,
            database_name: database_name.to_string(),
            temporary: false,
        }
    }
}

/// Manages a PostgreSQL sidecar instance.
///
/// Spawns `initdb`, `pg_ctl`, and `pg_isready` as child processes.
/// Data persists across restarts unless `temporary` is set.
pub struct LocalDbManager {
    config: PgConfig,
    started: bool,
    /// Holds the tempdir so it lives as long as LocalDbManager (dropped = cleaned up).
    _tempdir: Option<tempfile::TempDir>,
}

impl LocalDbManager {
    /// Creates a new `LocalDbManager` with the given configuration.
    pub fn new(config: PgConfig) -> Self {
        Self {
            config,
            started: false,
            _tempdir: None,
        }
    }

    /// Creates a new `LocalDbManager` using the platform-appropriate application data directory.
    ///
    /// PG binaries discovered via `pg_config` on PATH.
    /// Data is stored at `$APP_DATA/nize/pgdata/`.
    pub async fn with_default_data_dir() -> Result<Self> {
        let data_dir = default_data_dir().ok_or(DbError::NoDataDir)?;
        let config = PgConfig::from_env(data_dir, DEFAULT_DATABASE).await?;
        Ok(Self::new(config))
    }

    /// Creates a new `LocalDbManager` with ephemeral (temporary) storage for testing.
    ///
    /// PG binaries discovered via `pg_config` on PATH.
    /// Data is cleaned up when the `LocalDbManager` is dropped.
    pub async fn ephemeral() -> Result<Self> {
        let tempdir = tempfile::tempdir()?;
        let data_dir = tempdir.path().join("pgdata");

        let mut config = PgConfig::from_env(data_dir, DEFAULT_DATABASE).await?;
        config.temporary = true;

        Ok(Self {
            config,
            started: false,
            _tempdir: Some(tempdir),
        })
    }

    /// Performs first-time setup: initializes the PostgreSQL data directory.
    ///
    /// Safe to call on subsequent starts — skips if data directory already exists.
    pub async fn setup(&mut self) -> Result<()> {
        if self.config.data_dir.join("PG_VERSION").exists() {
            log::info!("Data directory already initialized, skipping initdb");
            return Ok(());
        }

        log::info!("Initializing PostgreSQL data directory...");
        let initdb = self.config.bin_dir.join("initdb");
        let mut cmd = Command::new(&initdb);
        cmd.arg("-D")
            .arg(&self.config.data_dir)
            .arg("--no-locale")
            .arg("--encoding=UTF8");
        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DbError::Command(format!("initdb failed: {stderr}")));
        }

        log::info!("PostgreSQL data directory initialized");
        Ok(())
    }

    /// Starts the PostgreSQL server and ensures the application database and
    /// pgvector extension exist.
    pub async fn start(&mut self) -> Result<()> {
        // Assign a free port if port is 0
        if self.config.port == 0 {
            self.config.port = find_free_port()?;
        }

        let pg_ctl = self.config.bin_dir.join("pg_ctl");

        // Stop any stale server left over from a previous unclean shutdown.
        let mut status_cmd = Command::new(&pg_ctl);
        status_cmd
            .arg("-D")
            .arg(&self.config.data_dir)
            .arg("status");
        let status = status_cmd.output().await?;

        if status.status.success() {
            log::info!("Stale PostgreSQL server detected — stopping before restart");
            let mut stop_cmd = Command::new(&pg_ctl);
            stop_cmd
                .arg("-D")
                .arg(&self.config.data_dir)
                .arg("-m")
                .arg("fast")
                .arg("stop");
            let stop = stop_cmd.output().await?;

            if !stop.status.success() {
                let stderr = String::from_utf8_lossy(&stop.stderr);
                return Err(DbError::Command(format!(
                    "pg_ctl stop (stale) failed: {stderr}"
                )));
            }
        }

        log::info!("Starting PostgreSQL on port {}...", self.config.port);

        let port_opt = format!(
            "-p {} -k \"{}\" -h localhost",
            self.config.port,
            self.config.data_dir.display()
        );
        let logfile = self.config.data_dir.join("postgresql.log");

        let mut start_cmd = Command::new(&pg_ctl);
        start_cmd
            .arg("-D")
            .arg(&self.config.data_dir)
            .arg("-o")
            .arg(&port_opt)
            .arg("-l")
            .arg(&logfile)
            .arg("start");
        let output = start_cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DbError::Command(format!("pg_ctl start failed: {stderr}")));
        }

        // Wait for PG to become ready
        self.wait_for_ready().await?;
        self.started = true;

        log::info!("PostgreSQL started on port {}", self.config.port);

        // Provision the application database (create if missing, enable extensions).
        let provisioner = DbProvisioner::new(
            &self.connection_url(),
            &self.config.database_name,
            self.config.port,
        );
        provisioner.provision(ProvisionMode::Strict).await?;

        log::info!(
            "Database '{}' ready at {}",
            self.config.database_name,
            self.connection_url()
        );
        Ok(())
    }

    /// Stops the PostgreSQL server gracefully.
    pub async fn stop(&mut self) -> Result<()> {
        if !self.started {
            return Ok(());
        }

        log::info!("Stopping PostgreSQL...");

        let pg_ctl = self.config.bin_dir.join("pg_ctl");
        let mut cmd = Command::new(&pg_ctl);
        cmd.arg("-D")
            .arg(&self.config.data_dir)
            .arg("-m")
            .arg("fast")
            .arg("stop");
        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DbError::Command(format!("pg_ctl stop failed: {stderr}")));
        }

        self.started = false;
        log::info!("PostgreSQL stopped");
        Ok(())
    }

    /// Returns the PostgreSQL connection URL for the application database.
    pub fn connection_url(&self) -> String {
        format!(
            "postgresql://localhost:{}/{}",
            self.config.port, self.config.database_name
        )
    }

    /// Returns the port the server is listening on (0 if not yet assigned).
    pub fn port(&self) -> u16 {
        self.config.port
    }

    /// Returns whether the server has been started.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Returns a shell command string that will stop this PostgreSQL instance.
    ///
    /// Suitable for writing to a cleanup manifest (e.g. for `nize_terminator`).
    /// The command uses `pg_ctl -D <data_dir> -m fast stop`.
    pub fn pg_ctl_stop_command(&self) -> String {
        format!(
            "{} -D {} -m fast stop",
            shell_escape(self.config.bin_dir.join("pg_ctl").display().to_string()),
            shell_escape(self.config.data_dir.display().to_string()),
        )
    }

    /// Wait for PostgreSQL to become ready, polling `pg_isready`.
    async fn wait_for_ready(&self) -> Result<()> {
        let pg_isready = self.config.bin_dir.join("pg_isready");
        let deadline = tokio::time::Instant::now() + PG_READY_TIMEOUT;

        loop {
            let cmd = Command::new(&pg_isready);
            let mut cmd = cmd;
            cmd.arg("-p")
                .arg(self.config.port.to_string())
                .arg("-h")
                .arg("localhost");
            let output = cmd.output().await?;

            if output.status.success() {
                return Ok(());
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(DbError::ReadyTimeout(PG_READY_TIMEOUT));
            }

            sleep(PG_READY_POLL).await;
        }
    }

    /// Returns a reference to the PG configuration.
    pub fn config(&self) -> &PgConfig {
        &self.config
    }
}

// @zen-component: PLAN-007-PgLiteManager
/// Manages a PGlite instance running inside a Node.js process via pglite-socket.
///
/// Spawns `node pglite-server.mjs` and exposes the standard PG wire protocol on
/// `localhost:<port>`. The `nize_desktop_server` connects via `sqlx::PgPool` unchanged.
pub struct PgLiteManager {
    /// Path to the PGlite data directory.
    data_dir: PathBuf,
    /// Port the PGlite socket server is listening on (0 until started).
    port: u16,
    /// Database name (informational — PGlite runs single-db).
    database_name: String,
    /// PID of the Node.js child process (set after start).
    child_pid: Option<u32>,
    /// Whether the server has been started.
    started: bool,
}

impl PgLiteManager {
    // @zen-impl: PLAN-007-3.1
    /// Creates a new `PgLiteManager`.
    pub fn new(data_dir: PathBuf, database_name: &str) -> Self {
        Self {
            data_dir,
            port: 0,
            database_name: database_name.to_string(),
            child_pid: None,
            started: false,
        }
    }

    /// Creates a new `PgLiteManager` using the platform-appropriate application data directory.
    ///
    /// Uses `nize/pglite-data` (separate from native PG's `nize/pgdata`).
    pub fn with_default_data_dir() -> Result<Self> {
        let data_dir = default_pglite_data_dir().ok_or(DbError::NoDataDir)?;
        Ok(Self::new(data_dir, DEFAULT_DATABASE))
    }

    // @zen-impl: PLAN-007-3.1
    /// Starts the PGlite server by spawning `node pglite-server.mjs`.
    ///
    /// Reads `{"port": N}` from stdout (sidecar protocol) and waits for the
    /// PG wire protocol to become ready.
    pub fn start(
        &mut self,
        node_bin: &std::path::Path,
        server_script: &std::path::Path,
    ) -> Result<()> {
        use std::io::{BufRead, BufReader};
        use std::process::{Command as StdCommand, Stdio};

        let port = find_free_port()?;

        log::info!(
            "Starting PGlite server on port {} (data: {})...",
            port,
            self.data_dir.display()
        );

        let mut child = StdCommand::new(node_bin)
            .arg(server_script)
            .arg(format!("--db={}", self.data_dir.display()))
            .arg(format!("--port={port}"))
            .arg(format!("--database={}", self.database_name))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| DbError::Command(format!("spawn pglite-server: {e}")))?;

        let pid = child.id();

        // Read the first line of stdout for {"port": N}.
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DbError::Command("no stdout from pglite-server".to_string()))?;
        let mut reader = BufReader::new(stdout);
        let mut first_line = String::new();
        reader
            .read_line(&mut first_line)
            .map_err(|e| DbError::Command(format!("read pglite-server stdout: {e}")))?;

        #[derive(serde::Deserialize)]
        struct Ready {
            port: u16,
        }

        let ready: Ready = serde_json::from_str(&first_line)
            .map_err(|e| DbError::Command(format!("parse pglite-server JSON: {e}")))?;

        self.port = ready.port;
        self.child_pid = Some(pid);
        self.started = true;

        log::info!("PGlite server ready on port {} (pid: {})", self.port, pid);

        Ok(())
    }

    // @zen-impl: PLAN-007-3.1
    /// Stops the PGlite server by killing the Node.js process.
    pub fn stop(&mut self) -> Result<()> {
        if !self.started {
            return Ok(());
        }

        if let Some(pid) = self.child_pid.take() {
            log::info!("Stopping PGlite server (pid: {pid})...");
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .output();
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .output();
            }
        }

        self.started = false;
        log::info!("PGlite server stopped");
        Ok(())
    }

    // @zen-impl: PLAN-007-3.1
    /// Returns the PostgreSQL connection URL for the PGlite instance.
    pub fn connection_url(&self) -> String {
        format!(
            "postgresql://localhost:{}/{}",
            self.port, self.database_name
        )
    }

    /// Returns the port the server is listening on (0 if not yet assigned).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns whether the server has been started.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Returns the PID of the Node.js child process.
    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    // @zen-impl: PLAN-007-3.1
    /// Returns a shell command string that will kill this PGlite instance.
    ///
    /// Suitable for writing to a cleanup manifest (e.g. for `nize_terminator`).
    pub fn kill_command(&self) -> Option<String> {
        self.child_pid.map(|pid| {
            #[cfg(unix)]
            {
                format!("kill {pid}")
            }
            #[cfg(windows)]
            {
                format!("taskkill /PID {pid} /F")
            }
        })
    }
}

/// Controls how provisioning errors are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionMode {
    /// Errors are fatal — propagated to the caller.
    Strict,
    /// Errors are logged as warnings and swallowed.
    Lenient,
}

/// Provisions a PostgreSQL database: creates the application database if
/// missing and enables required extensions (e.g. `vector`).
///
/// Works against any reachable PostgreSQL instance (local or remote).
pub struct DbProvisioner {
    /// Connection URL for the application database.
    connection_url: String,
    /// Database name to create if missing.
    database_name: String,
    /// Port of the PostgreSQL instance (used for the maintenance connection).
    port: u16,
}

impl DbProvisioner {
    /// Creates a new provisioner.
    pub fn new(connection_url: &str, database_name: &str, port: u16) -> Self {
        Self {
            connection_url: connection_url.to_string(),
            database_name: database_name.to_string(),
            port,
        }
    }

    /// Creates a provisioner from a full connection URL, extracting the database
    /// name and port from the URL.
    pub fn from_url(connection_url: &str) -> std::result::Result<Self, String> {
        let url: url::Url = connection_url
            .parse()
            .map_err(|e| format!("invalid connection URL: {e}"))?;
        let port = url.port().unwrap_or(5432);
        let database_name = url.path().trim_start_matches('/').to_string();
        if database_name.is_empty() {
            return Err("connection URL has no database name".into());
        }
        Ok(Self {
            connection_url: connection_url.to_string(),
            database_name,
            port,
        })
    }

    /// Run provisioning: create the database if missing, then enable extensions.
    pub async fn provision(&self, mode: ProvisionMode) -> Result<()> {
        if let Err(e) = self.create_database_if_missing().await {
            match mode {
                ProvisionMode::Strict => return Err(e),
                ProvisionMode::Lenient => {
                    log::warn!("Provisioning: create database failed (lenient): {e}");
                }
            }
        }

        if let Err(e) = self.enable_vector_extension().await {
            match mode {
                ProvisionMode::Strict => return Err(e),
                ProvisionMode::Lenient => {
                    log::warn!("Provisioning: enable vector extension failed (lenient): {e}");
                }
            }
        }

        Ok(())
    }

    /// Create the application database if it doesn't exist.
    async fn create_database_if_missing(&self) -> Result<()> {
        // Connect to the default `postgres` database to check/create our database
        let maintenance_url = format!("postgresql://localhost:{}/postgres", self.port);
        let pool = PgPool::connect(&maintenance_url).await?;

        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
                .bind(&self.database_name)
                .fetch_one(&pool)
                .await?;

        if !exists {
            log::info!("Creating database '{}'...", self.database_name);
            // CREATE DATABASE cannot use bind parameters
            let sql = format!("CREATE DATABASE \"{}\"", self.database_name);
            sqlx::query(&sql).execute(&pool).await?;
        }

        pool.close().await;
        Ok(())
    }

    /// Enable the `vector` extension in the application database.
    async fn enable_vector_extension(&self) -> Result<()> {
        let pool = PgPool::connect(&self.connection_url).await?;

        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&pool)
            .await?;

        pool.close().await;
        Ok(())
    }
}

/// Find a free ephemeral port by binding to port 0.
fn find_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

/// Returns the default data directory for the PostgreSQL instance.
///
/// Platform paths:
/// - macOS: `~/Library/Application Support/nize/pgdata`
/// - Linux: `~/.local/share/nize/pgdata`
/// - Windows: `%APPDATA%\nize\pgdata`
pub fn default_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("nize").join("pgdata"))
}

/// Returns the default data directory for PGlite (separate from native PG).
///
/// Platform paths:
/// - macOS: `~/Library/Application Support/nize/pglite-data`
/// - Linux: `~/.local/share/nize/pglite-data`
/// - Windows: `%APPDATA%\nize\pglite-data`
pub fn default_pglite_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("nize").join("pglite-data"))
}

/// Wraps a string in single quotes for shell safety if it contains spaces or
/// special characters. Single quotes within the value are escaped.
// @zen-impl: PLAN-006-3.4
#[cfg(unix)]
fn shell_escape(s: String) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.')
    {
        s
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[test]
    fn default_data_dir_is_some() {
        let dir = default_data_dir();
        assert!(dir.is_some());
        let dir = dir.unwrap();
        assert!(dir.ends_with("nize/pgdata") || dir.ends_with("nize\\pgdata"));
    }

    #[tokio::test]
    async fn ephemeral_manager_has_zero_port() {
        let mgr = LocalDbManager::ephemeral()
            .await
            .expect("ephemeral LocalDbManager");
        assert_eq!(0, mgr.port());
    }

    #[tokio::test]
    async fn lifecycle_setup_start_stop() -> Result<()> {
        let mut mgr = LocalDbManager::ephemeral().await?;

        mgr.setup().await?;
        assert!(!mgr.is_started());

        mgr.start().await?;
        assert!(mgr.is_started());
        assert_ne!(0, mgr.port());

        // Verify connection URL is well-formed
        let url = mgr.connection_url();
        assert!(url.starts_with("postgresql://"));
        assert!(url.contains("nize"));

        mgr.stop().await?;
        assert!(!mgr.is_started());

        Ok(())
    }

    #[tokio::test]
    async fn vector_extension_is_available() -> Result<()> {
        let mut mgr = LocalDbManager::ephemeral().await?;
        mgr.setup().await?;
        mgr.start().await?;

        // Connect to the application database and verify the extension
        let pool = PgPool::connect(mgr.connection_url().as_str())
            .await
            .expect("Failed to connect");

        let row = sqlx::query("SELECT extname FROM pg_extension WHERE extname = 'vector'")
            .fetch_one(&pool)
            .await
            .expect("vector extension not found");
        let extname: String = row.get("extname");
        assert_eq!("vector", extname);

        pool.close().await;
        mgr.stop().await?;

        Ok(())
    }
}
