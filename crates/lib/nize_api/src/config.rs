//! API server configuration.

/// Configuration for the API server.
#[derive(Clone, Debug)]
pub struct ApiConfig {
    /// Address to bind the HTTP listener (e.g. "127.0.0.1:3100").
    pub bind_addr: String,
    /// PostgreSQL connection URL.
    pub pg_connection_url: String,
}

impl ApiConfig {
    /// Reads configuration from environment variables with sensible defaults.
    ///
    /// | Variable           | Default                                     |
    /// |--------------------|---------------------------------------------|
    /// | `BIND_ADDR`        | `127.0.0.1:3100`                            |
    /// | `DATABASE_URL`     | `postgres://localhost:5432/nize`             |
    pub fn from_env() -> Self {
        Self {
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:3100".into()),
            pg_connection_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://localhost:5432/nize".into()),
        }
    }
}
