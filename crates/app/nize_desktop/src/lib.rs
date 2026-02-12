use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use nize_api_client::Client as ApiClient;
use nize_core::db::LocalDbManager;
use serde::Deserialize;
use tauri::Manager;
use tracing::{error, info};

/// JSON payload the API sidecar prints to stdout on startup.
#[derive(Deserialize)]
struct SidecarReady {
    port: u16,
}

/// State shared across Tauri commands.
struct ApiSidecar {
    client: ApiClient,
    _process: Child,
}

/// Holds the managed PG instance and API sidecar for the app lifetime.
struct AppServices {
    sidecar: Option<ApiSidecar>,
    /// Held to keep the PG process alive (stopped when dropped via `pg_ctl stop`).
    _db: Option<LocalDbManager>,
    /// nize_terminator child process (killed on graceful exit).
    terminator: Option<Child>,
    /// Path to the cleanup manifest file.
    manifest_path: Option<PathBuf>,
}

/// Spawns the `nize_api_server` binary and reads the port from its JSON stdout line.
fn start_api_sidecar(database_url: &str) -> Result<ApiSidecar, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let sidecar_path = exe.parent().ok_or("no parent dir")?.join("nize_api_server");

    info!(path = %sidecar_path.display(), "starting API sidecar");

    let mut child = Command::new(&sidecar_path)
        .arg("--port")
        .arg("0")
        .arg("--database-url")
        .arg(database_url)
        .arg("--sidecar")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("spawn sidecar: {e}"))?;

    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut reader = std::io::BufReader::new(stdout);
    let mut first_line = String::new();
    reader
        .read_line(&mut first_line)
        .map_err(|e| format!("read sidecar stdout: {e}"))?;

    let ready: SidecarReady =
        serde_json::from_str(&first_line).map_err(|e| format!("parse sidecar JSON: {e}"))?;

    info!(port = ready.port, "API sidecar ready");

    let client = ApiClient::new(&format!("http://127.0.0.1:{}", ready.port));

    Ok(ApiSidecar {
        client,
        _process: child,
    })
}

// @zen-impl: PLAN-005 — manifest path helper
/// Returns the manifest file path: `$TMPDIR/nize-<pid>-cleanup.manifest`.
fn manifest_path() -> PathBuf {
    let pid = std::process::id();
    std::env::temp_dir().join(format!("nize-{pid}-cleanup.manifest"))
}

// @zen-impl: PLAN-005 — create manifest and spawn terminator
/// Creates an empty manifest file and spawns `nize_terminator` watching our PID.
fn create_manifest_and_spawn_terminator(manifest: &Path) -> Result<Child, String> {
    // Create (or truncate) the manifest file.
    File::create(manifest).map_err(|e| format!("create manifest: {e}"))?;

    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let terminator_path = exe.parent().ok_or("no parent dir")?.join("nize_terminator");

    let pid = std::process::id();
    let child = Command::new(&terminator_path)
        .arg("--parent-pid")
        .arg(pid.to_string())
        .arg("--manifest")
        .arg(manifest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("spawn nize_terminator: {e}"))?;

    Ok(child)
}

// @zen-impl: PLAN-005 — atomic append to manifest
/// Appends a cleanup command line to the manifest file (atomic append + fsync).
fn append_cleanup(manifest: &Path, cmd: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .append(true)
        .open(manifest)
        .map_err(|e| format!("open manifest for append: {e}"))?;

    writeln!(file, "{cmd}").map_err(|e| format!("write to manifest: {e}"))?;
    file.flush().map_err(|e| format!("flush manifest: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("fsync manifest: {e}"))?;

    Ok(())
}

#[tauri::command]
async fn hello_world(
    state: tauri::State<'_, Mutex<AppServices>>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.lock().map_err(|e| format!("lock: {e}"))?;
        match &guard.sidecar {
            Some(s) => s.client.clone(),
            None => return Err("API sidecar not running".into()),
        }
    };

    let resp = client
        .hello_hello_world()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    let body =
        serde_json::to_value(resp.into_inner()).map_err(|e| format!("serialize response: {e}"))?;

    Ok(body)
}

pub fn run() {
    // Initialize logging so LocalDbManager (log crate) and tracing messages are visible.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,nize_core=debug".parse().unwrap()),
        )
        .init();

    // @zen-impl: PLAN-005 — spawn terminator before managed processes
    // 1. Create empty manifest file.
    // 2. Spawn nize_terminator watching our PID.
    // 3. Start DB, append cleanup command to manifest.
    // 4. Start API sidecar.
    let manifest_path = manifest_path();
    let terminator = match create_manifest_and_spawn_terminator(&manifest_path) {
        Ok(child) => {
            info!(pid = child.id(), "nize_terminator spawned");
            Some(child)
        }
        Err(e) => {
            error!("Failed to spawn nize_terminator: {e}");
            None
        }
    };

    // Start managed PostgreSQL and the API sidecar before the Tauri event loop.
    let services = tauri::async_runtime::block_on(async {
        match LocalDbManager::with_default_data_dir().await {
            Ok(mut db) => {
                if let Err(e) = db.setup().await {
                    error!("DB setup failed: {e}");
                    return AppServices {
                        sidecar: None,
                        _db: None,
                        terminator,
                        manifest_path: Some(manifest_path.clone()),
                    };
                }
                if let Err(e) = db.start().await {
                    error!("DB start failed: {e}");
                    return AppServices {
                        sidecar: None,
                        _db: None,
                        terminator,
                        manifest_path: Some(manifest_path.clone()),
                    };
                }

                // @zen-impl: PLAN-005 — append PG cleanup command to manifest
                let stop_cmd = db.pg_ctl_stop_command();
                if let Err(e) = append_cleanup(&manifest_path, &stop_cmd) {
                    error!("Failed to write cleanup command to manifest: {e}");
                }

                let db_url = db.connection_url();
                info!(url = %db_url, "PostgreSQL started");

                let sidecar = match start_api_sidecar(&db_url) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        error!("Failed to start API sidecar: {e}");
                        None
                    }
                };

                AppServices {
                    sidecar,
                    _db: Some(db),
                    terminator,
                    manifest_path: Some(manifest_path),
                }
            }
            Err(e) => {
                error!("Failed to create LocalDbManager: {e}");
                AppServices {
                    sidecar: None,
                    _db: None,
                    terminator,
                    manifest_path: Some(manifest_path),
                }
            }
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(services))
        .invoke_handler(tauri::generate_handler![hello_world])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                info!("Tauri exit — shutting down services");
                let state = app.state::<Mutex<AppServices>>();
                if let Ok(mut guard) = state.lock() {
                    // Drop the sidecar first so it releases PG connections.
                    guard.sidecar.take();

                    // Stop PostgreSQL synchronously before the process exits.
                    if let Some(mut db) = guard._db.take() {
                        tauri::async_runtime::block_on(async move {
                            if let Err(e) = db.stop().await {
                                error!("Failed to stop PostgreSQL: {e}");
                            }
                        });
                    }

                    // @zen-impl: PLAN-005 — kill terminator and delete manifest on graceful exit
                    if let Some(mut terminator) = guard.terminator.take() {
                        if let Err(e) = terminator.kill() {
                            // Expected if terminator already exited (e.g. parent-death race).
                            info!("Terminator kill (expected if already exited): {e}");
                        }
                    }
                    if let Some(ref path) = guard.manifest_path {
                        if path.exists() {
                            if let Err(e) = fs::remove_file(path) {
                                error!("Failed to remove manifest: {e}");
                            }
                        }
                    }
                }
            }
        });
}
