use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use nize_api_client::Client as ApiClient;
use nize_core::db::PgLiteManager;
use serde::Deserialize;
use tauri::Manager;
use tracing::{error, info};

mod mcp_clients;

/// JSON payload the API sidecar prints to stdout on startup.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SidecarReady {
    port: u16,
    mcp_port: u16,
}

// @zen-impl: PLAN-012-3.1 — nize-web sidecar ready payload
/// JSON payload the nize-web sidecar prints to stdout on startup.
#[derive(Deserialize)]
struct NizeWebReady {
    port: u16,
}

/// State shared across Tauri commands.
struct ApiSidecar {
    client: ApiClient,
    _process: Child,
    /// Bound port of the API sidecar (for frontend direct access).
    port: u16,
    /// Bound port of the MCP server.
    mcp_port: u16,
}

// @zen-impl: PLAN-012-3.1 — nize-web sidecar state
/// Holds the nize-web child process and its bound port.
struct NizeWebSidecar {
    _process: Child,
    port: u16,
}

/// Holds the managed PGlite instance and API sidecar for the app lifetime.
struct AppServices {
    sidecar: Option<ApiSidecar>,
    /// nize-web sidecar (Next.js hello world app).
    nize_web: Option<NizeWebSidecar>,
    /// Held to keep the PGlite process alive (killed when stopped).
    _pglite: Option<PgLiteManager>,
    /// nize_terminator child process (killed on graceful exit).
    terminator: Option<Child>,
    /// Path to the cleanup manifest file.
    manifest_path: Option<PathBuf>,
}

/// Spawns the `nize_desktop_server` binary and reads the port from its JSON stdout line.
fn start_api_sidecar(database_url: &str, max_connections: u32) -> Result<ApiSidecar, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let sidecar_path = exe
        .parent()
        .ok_or("no parent dir")?
        .join("nize_desktop_server");

    // MCP port: honour NIZE_MCP_PORT env var, default 19560.
    let mcp_port_arg = std::env::var("NIZE_MCP_PORT").unwrap_or_else(|_| "19560".to_string());

    info!(path = %sidecar_path.display(), "starting API sidecar");

    let mut child = Command::new(&sidecar_path)
        .arg("--port")
        .arg("0")
        .arg("--mcp-port")
        .arg(&mcp_port_arg)
        .arg("--database-url")
        .arg(database_url)
        .arg("--max-connections")
        .arg(max_connections.to_string())
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

    info!(
        port = ready.port,
        mcp_port = ready.mcp_port,
        "API sidecar ready"
    );

    let client = ApiClient::new(&format!("http://127.0.0.1:{}", ready.port));

    Ok(ApiSidecar {
        client,
        _process: child,
        port: ready.port,
        mcp_port: ready.mcp_port,
    })
}

// @zen-impl: PLAN-012-3.2 — spawn nize-web sidecar
/// Spawns `node nize-web-server.mjs --port=0` and reads the port from its JSON stdout line.
fn start_nize_web_sidecar(
    node_bin: &Path,
    server_script: &Path,
    api_port: Option<u16>,
) -> Result<NizeWebSidecar, String> {
    info!(script = %server_script.display(), "starting nize-web sidecar");

    let mut cmd = Command::new(node_bin);
    cmd.arg(server_script).arg("--port=0");

    // @zen-impl: CFG-NizeWebApi — pass API port so nize-web can reach the backend
    if let Some(p) = api_port {
        cmd.arg(format!("--api-port={p}"));
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("spawn nize-web: {e}"))?;

    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut reader = std::io::BufReader::new(stdout);
    let mut first_line = String::new();
    reader
        .read_line(&mut first_line)
        .map_err(|e| format!("read nize-web stdout: {e}"))?;

    let ready: NizeWebReady =
        serde_json::from_str(&first_line).map_err(|e| format!("parse nize-web JSON: {e}"))?;

    info!(port = ready.port, "nize-web sidecar ready");

    Ok(NizeWebSidecar {
        _process: child,
        port: ready.port,
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

#[tauri::command]
async fn get_api_port(state: tauri::State<'_, Mutex<AppServices>>) -> Result<u16, String> {
    let guard = state.lock().map_err(|e| format!("lock: {e}"))?;
    match &guard.sidecar {
        Some(s) => Ok(s.port),
        None => Err("API sidecar not running".into()),
    }
}

#[tauri::command]
async fn get_mcp_port(state: tauri::State<'_, Mutex<AppServices>>) -> Result<u16, String> {
    let guard = state.lock().map_err(|e| format!("lock: {e}"))?;
    match &guard.sidecar {
        Some(s) => Ok(s.mcp_port),
        None => Err("API sidecar not running".into()),
    }
}

// @zen-impl: PLAN-012-3.5 — Tauri command to expose nize-web port to frontend
#[tauri::command]
async fn get_nize_web_port(state: tauri::State<'_, Mutex<AppServices>>) -> Result<u16, String> {
    let guard = state.lock().map_err(|e| format!("lock: {e}"))?;
    match &guard.nize_web {
        Some(s) => Ok(s.port),
        None => Err("nize-web sidecar not running".into()),
    }
}

pub fn run() {
    // Initialize logging so PgLiteManager (log crate) and tracing messages are visible.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,nize_core=debug".parse().unwrap()),
        )
        .init();

    // @zen-impl: PLAN-005 — spawn terminator before managed processes
    // 1. Create empty manifest file.
    // 2. Spawn nize_terminator watching our PID.
    // 3. Start PGlite, append cleanup command to manifest.
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

    // External database override via environment variable.
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        info!(url = %db_url, "Using DATABASE_URL from environment");

        let sidecar = match start_api_sidecar(&db_url, 5) {
            Ok(s) => Some(s),
            Err(e) => {
                error!("Failed to start API sidecar: {e}");
                None
            }
        };

        return run_tauri(AppServices {
            sidecar,
            nize_web: None,
            _pglite: None,
            terminator,
            manifest_path: Some(manifest_path),
        });
    }

    // @zen-impl: PLAN-007-5.1 — start PGlite and the API sidecar before the Tauri event loop.
    let services = {
        let exe = std::env::current_exe().expect("current_exe");
        let exe_dir = exe.parent().expect("exe parent dir");

        // Resolve node binary: bundled externalBin or PATH fallback.
        let node_bin = {
            let bundled = exe_dir.join("node");
            if bundled.exists() {
                bundled
            } else {
                PathBuf::from("node")
            }
        };

        // @zen-impl: PLAN-007-5.1 — resolve pglite-server.mjs from resources.
        let server_script = {
            // Production macOS .app: Contents/MacOS/exe → Contents/Resources/pglite/
            let resource = exe_dir
                .parent()
                .map(|p| p.join("Resources").join("pglite").join("pglite-server.mjs"));
            match resource {
                Some(ref p) if p.exists() => p.clone(),
                _ => {
                    // Dev fallback: look in the nize_desktop resources directory.
                    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("resources")
                        .join("pglite")
                        .join("pglite-server.mjs");
                    dev_path
                }
            }
        };

        if !server_script.exists() {
            error!(
                "pglite-server.mjs not found at {}; set DATABASE_URL to use an external database",
                server_script.display()
            );
            return run_tauri(AppServices {
                sidecar: None,
                nize_web: None,
                _pglite: None,
                terminator,
                manifest_path: Some(manifest_path),
            });
        }

        // PGlite mode: spawn node pglite-server.mjs.
        let mut pglite = match PgLiteManager::with_default_data_dir() {
            Ok(mgr) => mgr,
            Err(e) => {
                error!("Failed to create PgLiteManager: {e}");
                return run_tauri(AppServices {
                    sidecar: None,
                    nize_web: None,
                    _pglite: None,
                    terminator,
                    manifest_path: Some(manifest_path),
                });
            }
        };

        if let Err(e) = pglite.start(&node_bin, &server_script) {
            error!("PGlite start failed: {e}");
            return run_tauri(AppServices {
                sidecar: None,
                nize_web: None,
                _pglite: None,
                terminator,
                manifest_path: Some(manifest_path),
            });
        }

        // @zen-impl: PLAN-007-5.2 — append PGlite kill command to terminator manifest.
        if let Some(kill_cmd) = pglite.kill_command() {
            if let Err(e) = append_cleanup(&manifest_path, &kill_cmd) {
                error!("Failed to write cleanup command to manifest: {e}");
            }
        }

        let db_url = pglite.connection_url();
        info!(url = %db_url, "PGlite started");

        let sidecar = match start_api_sidecar(&db_url, 1) {
            Ok(s) => Some(s),
            Err(e) => {
                error!("Failed to start API sidecar: {e}");
                None
            }
        };

        // @zen-impl: PLAN-012-3.4 — start nize-web sidecar after API sidecar
        let nize_web_script = {
            let resource = exe_dir.parent().map(|p| {
                p.join("Resources")
                    .join("nize-web")
                    .join("nize-web-server.mjs")
            });
            match resource {
                Some(ref p) if p.exists() => p.clone(),
                _ => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("resources")
                    .join("nize-web")
                    .join("nize-web-server.mjs"),
            }
        };

        let nize_web = if nize_web_script.exists() {
            let api_port = sidecar.as_ref().map(|s| s.port);
            match start_nize_web_sidecar(&node_bin, &nize_web_script, api_port) {
                Ok(s) => {
                    // Append kill command to terminator manifest.
                    let kill_cmd = format!("kill {}", s._process.id());
                    if let Err(e) = append_cleanup(&manifest_path, &kill_cmd) {
                        error!("Failed to write nize-web cleanup to manifest: {e}");
                    }
                    Some(s)
                }
                Err(e) => {
                    error!("Failed to start nize-web sidecar: {e}");
                    None
                }
            }
        } else {
            info!("nize-web-server.mjs not found — skipping nize-web sidecar");
            None
        };

        AppServices {
            sidecar,
            nize_web,
            _pglite: Some(pglite),
            terminator,
            manifest_path: Some(manifest_path),
        }
    };

    run_tauri(services);
}

// @zen-impl: PLAN-007-5.3
fn run_tauri(services: AppServices) {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(Mutex::new(services))
        .invoke_handler(tauri::generate_handler![
            hello_world,
            get_api_port,
            get_mcp_port,
            get_nize_web_port,
            mcp_clients::get_mcp_client_statuses,
            mcp_clients::configure_mcp_client,
            mcp_clients::remove_mcp_client
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                if let Some(win) = app.get_webview_window("main") {
                    win.open_devtools();
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                info!("Tauri exit — shutting down services");
                let state = app.state::<Mutex<AppServices>>();
                if let Ok(mut guard) = state.lock() {
                    // Drop the sidecar first so it releases PG connections.
                    guard.sidecar.take();

                    // @zen-impl: PLAN-012-3.7 — kill nize-web sidecar on exit
                    guard.nize_web.take();

                    // @zen-impl: PLAN-007-5.3 — stop PGlite on exit.
                    if let Some(mut pglite) = guard._pglite.take() {
                        if let Err(e) = pglite.stop() {
                            error!("Failed to stop PGlite: {e}");
                        }
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
