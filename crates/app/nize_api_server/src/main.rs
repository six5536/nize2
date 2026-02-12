//! Nize API sidecar server binary.
//!
//! Started by the Tauri desktop app as a child process.
//! Prints `{"port": N}` to stdout so the parent can discover the bound port.

use clap::Parser;
use sqlx::postgres::PgPoolOptions;
use tracing::info;

/// CLI arguments for the API sidecar.
#[derive(Parser, Debug)]
#[command(name = "nize_api_server", about = "Nize API sidecar server")]
struct Args {
    /// Port to listen on (0 = ephemeral).
    #[arg(long, default_value_t = 0)]
    port: u16,

    /// PostgreSQL connection URL.
    #[arg(
        long,
        env = "DATABASE_URL",
        default_value = "postgres://localhost:5432/nize"
    )]
    database_url: String,

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

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&args.database_url)
        .await?;

    let config = nize_api::config::ApiConfig {
        bind_addr: format!("127.0.0.1:{}", args.port),
        pg_connection_url: args.database_url,
    };

    let state = nize_api::AppState {
        pool,
        config: config.clone(),
    };

    let app = nize_api::router(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    let local_addr = listener.local_addr()?;

    // Report the bound port as JSON on stdout so the parent process (Tauri) can read it.
    println!("{}", serde_json::json!({"port": local_addr.port()}));

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

    info!(addr = %local_addr, "listening");
    axum::serve(listener, app).await?;

    Ok(())
}
