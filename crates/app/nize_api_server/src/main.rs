//! Nize API sidecar server binary.
//!
//! Started by the Tauri desktop app as a child process.
//! Prints `{"port": N}` to stdout so the parent can discover the bound port.

use clap::Parser;
use sqlx::postgres::PgPoolOptions;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// CLI arguments for the API sidecar.
#[derive(Parser, Debug)]
#[command(name = "nize_api_server", about = "Nize API sidecar server")]
struct Args {
    /// Port to listen on (0 = ephemeral).
    #[arg(long, default_value_t = 0)]
    port: u16,

    /// MCP server port (0 = ephemeral).
    #[arg(long, default_value_t = 0)]
    mcp_port: u16,

    /// PostgreSQL connection URL.
    #[arg(
        long,
        env = "DATABASE_URL",
        default_value = "postgres://localhost:5432/nize"
    )]
    database_url: String,

    /// Maximum number of database connections in the pool.
    ///
    /// Set to 1 when the backend is PGlite (single-connection only) so that
    /// concurrent requests queue at the pool level instead of failing.
    #[arg(long, default_value_t = 5)]
    max_connections: u32,

    /// Run as a managed sidecar: exit automatically when the parent process dies.
    ///
    /// When set, the server monitors stdin for EOF. The parent keeps the write
    /// end of the pipe open; if the parent exits (even via SIGKILL) the OS
    /// closes the pipe and the server shuts down.
    #[arg(long, default_value_t = false)]
    sidecar: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    // Write logs to stderr so stdout is reserved for the JSON port message.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,nize_api=debug,nize_core=debug".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    info!(database_url = %args.database_url, port = args.port, "starting nize_api_server");

    info!(
        max_connections = args.max_connections,
        sidecar = args.sidecar,
        "configuring connection pool"
    );

    let pool = PgPoolOptions::new()
        .max_connections(args.max_connections)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(&args.database_url)
        .await?;

    // Run database migrations.
    info!("running database migrations");
    nize_api::migrate(&pool).await?;

    let config = nize_api::config::ApiConfig {
        bind_addr: format!("127.0.0.1:{}", args.port),
        pg_connection_url: args.database_url,
        jwt_secret: nize_api::services::auth::resolve_jwt_secret(),
        mcp_encryption_key: std::env::var("MCP_ENCRYPTION_KEY")
            .unwrap_or_else(|_| "nize-mcp-default-dev-key-change-in-production".into()),
    };

    // Clone pool for MCP server before moving into API state.
    let mcp_pool = pool.clone();

    let config_cache = std::sync::Arc::new(tokio::sync::RwLock::new(
        nize_core::config::cache::ConfigCache::new(),
    ));

    let state = nize_api::AppState {
        pool,
        config: config.clone(),
        config_cache: config_cache.clone(),
        oauth_state: std::sync::Arc::new(nize_core::mcp::oauth::OAuthStateStore::new()),
    };

    let app = nize_api::router(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    let local_addr = listener.local_addr()?;

    // Build MCP server on a separate port.
    let mcp_ct = CancellationToken::new();
    let mcp_app = nize_mcp::mcp_router(mcp_pool, config_cache, mcp_ct.clone(), config.mcp_encryption_key.clone());
    let mcp_bind = format!("127.0.0.1:{}", args.mcp_port);
    let mcp_listener = tokio::net::TcpListener::bind(&mcp_bind).await?;
    let mcp_addr = mcp_listener.local_addr()?;

    // Report both bound ports as JSON on stdout so the parent process (Tauri) can read them.
    println!(
        "{}",
        serde_json::json!({"port": local_addr.port(), "mcpPort": mcp_addr.port()})
    );

    if args.sidecar {
        info!("sidecar mode: will exit when parent pipe closes");
        tokio::spawn(async {
            use tokio::io::AsyncReadExt;
            let mut stdin = tokio::io::stdin();
            let mut buf = [0u8; 1];
            // Blocks until the parent dies and the OS closes the pipe → EOF.
            let _ = stdin.read(&mut buf).await;
            info!("parent pipe closed, shutting down");
            std::process::exit(0);
        });
    }

    info!(addr = %local_addr, "REST API listening");
    info!(addr = %mcp_addr, "MCP server listening");

    // Spawn MCP server.
    let mcp_handle = tokio::spawn({
        let mcp_ct = mcp_ct.clone();
        async move {
            axum::serve(mcp_listener, mcp_app)
                .with_graceful_shutdown(async move { mcp_ct.cancelled().await })
                .await
        }
    });

    // Run REST API on the main task.
    let api_result = axum::serve(listener, app).await;

    // When the REST API exits, also cancel MCP.
    mcp_ct.cancel();
    let _ = mcp_handle.await;

    api_result?;

    Ok(())
}
