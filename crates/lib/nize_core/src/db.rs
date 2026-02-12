//! PostgreSQL sidecar database management.
//!
//! Provides `DbManager` for lifecycle management of a PostgreSQL instance
//! via process spawning (`initdb`, `pg_ctl`, `pg_isready`).

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
pub struct DbManager {
    config: PgConfig,
    started: bool,
    /// Holds the tempdir so it lives as long as DbManager (dropped = cleaned up).
    _tempdir: Option<tempfile::TempDir>,
}

impl DbManager {
    /// Creates a new `DbManager` with the given configuration.
    pub fn new(config: PgConfig) -> Self {
        Self {
            config,
            started: false,
            _tempdir: None,
        }
    }

    /// Creates a new `DbManager` using the platform-appropriate application data directory.
    ///
    /// PG binaries discovered via `pg_config` on PATH.
    /// Data is stored at `$APP_DATA/nize/pgdata/`.
    pub async fn with_default_data_dir() -> Result<Self> {
        let data_dir = default_data_dir().ok_or(DbError::NoDataDir)?;
        let config = PgConfig::from_env(data_dir, DEFAULT_DATABASE).await?;
        Ok(Self::new(config))
    }

    /// Creates a new `DbManager` with ephemeral (temporary) storage for testing.
    ///
    /// PG binaries discovered via `pg_config` on PATH.
    /// Data is cleaned up when the `DbManager` is dropped.
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
        let output = Command::new(&initdb)
            .arg("-D")
            .arg(&self.config.data_dir)
            .arg("--no-locale")
            .arg("--encoding=UTF8")
            .output()
            .await?;

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

        log::info!("Starting PostgreSQL on port {}...", self.config.port);

        let pg_ctl = self.config.bin_dir.join("pg_ctl");
        let port_opt = format!(
            "-p {} -k {} -h localhost",
            self.config.port,
            self.config.data_dir.display()
        );
        let logfile = self.config.data_dir.join("postgresql.log");

        let output = Command::new(&pg_ctl)
            .arg("-D")
            .arg(&self.config.data_dir)
            .arg("-o")
            .arg(&port_opt)
            .arg("-l")
            .arg(&logfile)
            .arg("start")
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DbError::Command(format!("pg_ctl start failed: {stderr}")));
        }

        // Wait for PG to become ready
        self.wait_for_ready().await?;
        self.started = true;

        log::info!("PostgreSQL started on port {}", self.config.port);

        // Create application database if it doesn't exist
        self.create_database_if_missing().await?;

        // Enable the vector extension in the application database
        self.enable_vector_extension().await?;

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
        let output = Command::new(&pg_ctl)
            .arg("-D")
            .arg(&self.config.data_dir)
            .arg("-m")
            .arg("fast")
            .arg("stop")
            .output()
            .await?;

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

    /// Wait for PostgreSQL to become ready, polling `pg_isready`.
    async fn wait_for_ready(&self) -> Result<()> {
        let pg_isready = self.config.bin_dir.join("pg_isready");
        let deadline = tokio::time::Instant::now() + PG_READY_TIMEOUT;

        loop {
            let output = Command::new(&pg_isready)
                .arg("-p")
                .arg(self.config.port.to_string())
                .arg("-h")
                .arg("localhost")
                .output()
                .await?;

            if output.status.success() {
                return Ok(());
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(DbError::ReadyTimeout(PG_READY_TIMEOUT));
            }

            sleep(PG_READY_POLL).await;
        }
    }

    /// Create the application database if it doesn't exist.
    async fn create_database_if_missing(&self) -> Result<()> {
        // Connect to the default `postgres` database to check/create our database
        let maintenance_url = format!("postgresql://localhost:{}/postgres", self.config.port);
        let pool = PgPool::connect(&maintenance_url).await?;

        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
                .bind(&self.config.database_name)
                .fetch_one(&pool)
                .await?;

        if !exists {
            log::info!("Creating database '{}'...", self.config.database_name);
            // CREATE DATABASE cannot use bind parameters
            let sql = format!("CREATE DATABASE \"{}\"", self.config.database_name);
            sqlx::query(&sql).execute(&pool).await?;
        }

        pool.close().await;
        Ok(())
    }

    /// Enable the `vector` extension in the application database.
    async fn enable_vector_extension(&self) -> Result<()> {
        let url = self.connection_url();
        let pool = PgPool::connect(&url).await?;

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
        let mgr = DbManager::ephemeral().await.expect("ephemeral DbManager");
        assert_eq!(0, mgr.port());
    }

    #[tokio::test]
    async fn lifecycle_setup_start_stop() -> Result<()> {
        let mut mgr = DbManager::ephemeral().await?;

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
        let mut mgr = DbManager::ephemeral().await?;
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
