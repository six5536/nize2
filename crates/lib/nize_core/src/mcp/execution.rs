// @zen-component: MCP-ExecutionProxy
//
//! Execution proxy — connects to external MCP servers and executes tools.
//!
//! Maintains a pool of `RunningService` connections keyed by server ID.
//! Supports HTTP (Streamable HTTP), SSE (legacy), stdio, and managed transports.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use sqlx::PgPool;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

use crate::models::mcp::{
    AuthType, HttpServerConfig, ManagedHttpServerConfig, McpToolSummary, ServerConfig,
    SseServerConfig, StdioServerConfig, TestConnectionResult, TransportType,
};

use super::McpError;
use super::queries;

/// Default timeout for tool execution (30 seconds).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default timeout for stdio server connection/initialization (30 seconds).
const STDIO_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default maximum number of concurrent managed processes.
const DEFAULT_MAX_MANAGED_PROCESSES: usize = 50;

/// Default idle timeout for managed connections (5 minutes).
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Interval between readiness probe retries.
const READY_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// OAuth credentials to pass when connecting to an authenticated MCP server.
#[derive(Debug, Clone)]
pub struct OAuthHeaders {
    /// Google ID token — sent as `Authorization: Bearer <id_token>`.
    pub id_token: String,
    /// Google access token — sent as `X-Google-Access-Token` header.
    pub access_token: String,
}

/// Request to execute a tool on an external MCP server.
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    pub tool_id: Uuid,
    pub tool_name: String,
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
    pub user_id: String,
}

/// Result of executing a tool on an external MCP server.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub tool_name: String,
    pub result: serde_json::Value,
}

/// Entry in the connection pool, tracking transport type alongside the service.
struct PoolEntry {
    service: RunningService<RoleClient, ()>,
    transport: TransportType,
    /// Milliseconds since pool epoch when this entry was last accessed.
    last_accessed: AtomicU64,
    /// When the entry was created.
    #[allow(dead_code)]
    created_at: Instant,
    /// Child process handle for managed transports (stdio, managed-sse, managed-http).
    /// Killed when the pool entry is removed/evicted.
    child_process: Option<tokio::process::Child>,
}

impl PoolEntry {
    /// Update the last-accessed timestamp (lock-free atomic store).
    fn touch(&self, epoch: &Instant) {
        let ms = epoch.elapsed().as_millis() as u64;
        self.last_accessed.store(ms, Ordering::Relaxed);
    }

    /// Duration since last access.
    fn idle_duration(&self, epoch: &Instant) -> Duration {
        let now_ms = epoch.elapsed().as_millis() as u64;
        let last = self.last_accessed.load(Ordering::Relaxed);
        Duration::from_millis(now_ms.saturating_sub(last))
    }
}

/// Client connection pool — reuses MCP client sessions across calls.
///
/// Keyed by server ID. Connections are lazily created and kept alive.
/// On error, the connection is removed and a retry is attempted.
/// Supports HTTP, SSE, stdio, and managed transports.
pub struct ClientPool {
    connections: Arc<DashMap<Uuid, PoolEntry>>,
    /// Guard set to prevent duplicate concurrent spawns for the same server.
    connecting: Arc<Mutex<HashSet<Uuid>>>,
    /// Path to the terminator manifest file for managed process PID registration.
    manifest_path: Option<PathBuf>,
    /// Maximum number of concurrent managed processes.
    max_managed_processes: usize,
    /// Idle timeout for stdio connections before eviction.
    idle_timeout: Duration,
    /// Reference point for atomic last-accessed timestamps.
    epoch: Instant,
}

impl ClientPool {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            connecting: Arc::new(Mutex::new(HashSet::new())),
            manifest_path: None,
            max_managed_processes: DEFAULT_MAX_MANAGED_PROCESSES,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            epoch: Instant::now(),
        }
    }

    /// Create a new pool with a terminator manifest path for stdio PID tracking.
    pub fn with_manifest(manifest_path: PathBuf) -> Self {
        Self {
            manifest_path: Some(manifest_path),
            ..Self::new()
        }
    }

    /// Set the maximum number of concurrent managed processes.
    pub fn set_max_managed_processes(&mut self, max: usize) {
        self.max_managed_processes = max;
    }

    /// Set the idle timeout for stdio connections.
    pub fn set_idle_timeout(&mut self, timeout: Duration) {
        self.idle_timeout = timeout;
    }

    /// Get the current idle timeout.
    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// Count current managed connections (stdio + managed-sse + managed-http).
    fn managed_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|entry| entry.value().transport.is_managed())
            .count()
    }

    // @zen-impl: PLAN-025 Phase 2.1 — atomic DashMap entry with connecting guard
    /// Get or create a connection to an MCP server.
    async fn get_or_connect(
        &self,
        pool: &PgPool,
        server_id: Uuid,
        oauth_headers: Option<&OAuthHeaders>,
    ) -> Result<(), McpError> {
        // Fast path: already connected.
        if let Some(entry) = self.connections.get(&server_id) {
            entry.touch(&self.epoch);
            return Ok(());
        }

        // Check if another task is already connecting for this server.
        {
            let mut guard = self.connecting.lock().await;
            if guard.contains(&server_id) {
                // Another task is spawning — wait briefly then retry the fast path.
                drop(guard);
                tokio::time::sleep(Duration::from_millis(100)).await;
                if self.connections.contains_key(&server_id) {
                    return Ok(());
                }
                return Err(McpError::ConnectionFailed(
                    "Connection already in progress for this server".into(),
                ));
            }
            guard.insert(server_id);
        }

        let result = self.connect(pool, server_id, oauth_headers).await;

        // Always remove from connecting guard.
        {
            let mut guard = self.connecting.lock().await;
            guard.remove(&server_id);
        }

        result
    }

    /// Internal connect logic — called within the connecting guard.
    async fn connect(
        &self,
        pool: &PgPool,
        server_id: Uuid,
        oauth_headers: Option<&OAuthHeaders>,
    ) -> Result<(), McpError> {
        let server = queries::get_server(pool, &server_id.to_string())
            .await?
            .ok_or_else(|| McpError::NotFound(format!("Server {server_id}")))?;

        let transport_type = server.transport.clone();

        // @zen-impl: PLAN-025 Phase 2.2 — match on transport type
        // @zen-impl: PLAN-033 T-XMCP-044 — dispatch all 5 transport types
        match transport_type {
            TransportType::Http => self.connect_http(&server, oauth_headers).await?,
            TransportType::Stdio => self.connect_stdio(&server, server_id).await?,
            TransportType::Sse => self.connect_sse(&server, oauth_headers).await?,
            TransportType::ManagedSse | TransportType::ManagedHttp => {
                self.connect_managed(&server, server_id, transport_type)
                    .await?
            }
        }

        Ok(())
    }

    /// Connect to an HTTP MCP server via Streamable HTTP transport.
    // @zen-impl: PLAN-031 Phase 7.1 — OAuth header support
    async fn connect_http(
        &self,
        server: &crate::models::mcp::McpServerRow,
        oauth_headers: Option<&OAuthHeaders>,
    ) -> Result<(), McpError> {
        // Parse the config to get the actual URL (endpoint column may be stale)
        let url = if let Some(ref config_json) = server.config {
            if let Ok(config) = serde_json::from_value::<ServerConfig>(config_json.clone()) {
                config.endpoint().to_string()
            } else {
                server.endpoint.clone()
            }
        } else {
            server.endpoint.clone()
        };

        // Build transport config
        let mut config = StreamableHttpClientTransportConfig::with_uri(&*url);

        // Determine auth type from server config
        let auth_type = queries::extract_auth_type(&server.config);

        let transport = if auth_type == AuthType::OAuth {
            // For OAuth servers, inject auth headers via custom reqwest::Client
            let headers = oauth_headers.ok_or_else(|| {
                McpError::ConnectionFailed(
                    "OAuth server requires authentication — please authorize first".into(),
                )
            })?;

            // Set id_token as Bearer auth (rmcp auto-adds "Bearer " prefix)
            config.auth_header = Some(headers.id_token.clone());

            // Build custom reqwest client with X-Google-Access-Token header
            let mut header_map = reqwest::header::HeaderMap::new();
            header_map.insert(
                reqwest::header::HeaderName::from_static("x-google-access-token"),
                reqwest::header::HeaderValue::from_str(&headers.access_token).map_err(|e| {
                    McpError::ConnectionFailed(format!("Invalid access token header value: {e}"))
                })?,
            );
            let client = reqwest::Client::builder()
                .default_headers(header_map)
                .build()
                .map_err(|e| {
                    McpError::ConnectionFailed(format!("Failed to build HTTP client: {e}"))
                })?;

            StreamableHttpClientTransport::with_client(client, config)
        } else {
            StreamableHttpClientTransport::from_config(config)
        };

        let service: RunningService<RoleClient, ()> = ().serve(transport).await.map_err(|e| {
            McpError::ConnectionFailed(format!(
                "Failed to connect to MCP server {}: {e}",
                server.name
            ))
        })?;

        self.connections.insert(
            server.id,
            PoolEntry {
                service,
                transport: TransportType::Http,
                last_accessed: AtomicU64::new(self.epoch.elapsed().as_millis() as u64),
                created_at: Instant::now(),
                child_process: None,
            },
        );
        Ok(())
    }

    // @zen-impl: PLAN-025 Phase 2.2 — stdio transport via TokioChildProcess
    /// Connect to a stdio MCP server by spawning its child process.
    async fn connect_stdio(
        &self,
        server: &crate::models::mcp::McpServerRow,
        server_id: Uuid,
    ) -> Result<(), McpError> {
        // @zen-impl: PLAN-025 Phase 2.3 — enforce max stdio process limit
        // @zen-impl: PLAN-030 Phase 3.2 — LRU eviction before ResourceExhausted
        // @zen-impl: PLAN-033 T-XMCP-052 — use is_managed() for managed process limit
        if self.managed_count() >= self.max_managed_processes && !self.evict_lru_managed() {
            return Err(McpError::ResourceExhausted(format!(
                "Maximum managed process limit ({}) reached",
                self.max_managed_processes
            )));
        }

        let config: StdioServerConfig = server
            .config
            .as_ref()
            .and_then(|c| serde_json::from_value(c.clone()).ok())
            .ok_or_else(|| {
                McpError::Validation(format!(
                    "Server \"{}\" has no valid stdio configuration",
                    server.name
                ))
            })?;

        let args = config.args.as_deref().unwrap_or_default();

        // @zen-impl: PLAN-025 Phase 4.4 — stderr inherits to server logs
        let mut cmd = tokio::process::Command::new(&config.command);
        cmd.args(args).stderr(std::process::Stdio::inherit());

        // Set environment variables if provided
        if let Some(env) = &config.env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        // @zen-impl: PLAN-025 Phase 4.3 — command not found maps to ConnectionFailed
        let transport = TokioChildProcess::new(cmd).map_err(|e| {
            McpError::ConnectionFailed(format!(
                "Failed to spawn stdio process '{}': {e}",
                config.command
            ))
        })?;

        // @zen-impl: PLAN-025 Phase 5.2 — write PID to terminator manifest
        if let Some(ref manifest) = self.manifest_path
            && let Some(pid) = transport.id()
            && let Err(e) = append_manifest(manifest, pid)
        {
            warn!("Failed to write stdio PID {pid} to manifest: {e}");
        }

        // @zen-impl: PLAN-025 Phase 4.2 — startup timeout for stdio servers
        let service: RunningService<RoleClient, ()> =
            tokio::time::timeout(STDIO_CONNECT_TIMEOUT, ().serve(transport))
                .await
                .map_err(|_| {
                    McpError::ConnectionFailed(format!(
                        "Stdio server '{}' did not respond within {}s",
                        server.name,
                        STDIO_CONNECT_TIMEOUT.as_secs()
                    ))
                })?
                .map_err(|e| {
                    McpError::ConnectionFailed(format!(
                        "Failed to initialize stdio MCP server {}: {e}",
                        server.name
                    ))
                })?;

        info!(
            server_name = %server.name,
            server_id = %server_id,
            "Stdio MCP server connected"
        );

        self.connections.insert(
            server_id,
            PoolEntry {
                service,
                transport: TransportType::Stdio,
                last_accessed: AtomicU64::new(self.epoch.elapsed().as_millis() as u64),
                created_at: Instant::now(),
                child_process: None, // TokioChildProcess manages its own child
            },
        );
        Ok(())
    }

    // @zen-impl: PLAN-033 T-XMCP-040 — connect to external SSE server
    /// Connect to an external SSE MCP server via the legacy SSE transport.
    async fn connect_sse(
        &self,
        server: &crate::models::mcp::McpServerRow,
        oauth_headers: Option<&OAuthHeaders>,
    ) -> Result<(), McpError> {
        let config: SseServerConfig = server
            .config
            .as_ref()
            .and_then(|c| {
                // Try parsing as ServerConfig first (has transport tag)
                if let Ok(sc) = serde_json::from_value::<ServerConfig>(c.clone()) {
                    if let ServerConfig::Sse(sse) = sc {
                        return Some(sse);
                    }
                }
                // Fallback: parse directly as SseServerConfig
                serde_json::from_value(c.clone()).ok()
            })
            .ok_or_else(|| {
                McpError::Validation(format!(
                    "Server \"{}\" has no valid SSE configuration",
                    server.name
                ))
            })?;

        let mut extra_headers = reqwest::header::HeaderMap::new();

        // Configure auth
        let auth_type = queries::extract_auth_type(&server.config);
        if auth_type == AuthType::OAuth {
            if let Some(headers) = oauth_headers {
                if let Ok(val) =
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {}", headers.id_token))
                {
                    extra_headers.insert(reqwest::header::AUTHORIZATION, val);
                }
                if let Ok(val) = reqwest::header::HeaderValue::from_str(&headers.access_token) {
                    extra_headers.insert(
                        reqwest::header::HeaderName::from_static("x-google-access-token"),
                        val,
                    );
                }
            }
        }

        // Add custom headers from config
        add_custom_headers(&mut extra_headers, &config.headers);

        let transport = super::sse_transport::SseClientTransport::with_client(
            self.build_sse_client(&extra_headers)?,
            &config.url,
            extra_headers,
        );

        let service: RunningService<RoleClient, ()> = ().serve(transport).await.map_err(|e| {
            McpError::ConnectionFailed(format!(
                "Failed to connect to SSE MCP server {}: {e}",
                server.name
            ))
        })?;

        self.connections.insert(
            server.id,
            PoolEntry {
                service,
                transport: TransportType::Sse,
                last_accessed: AtomicU64::new(self.epoch.elapsed().as_millis() as u64),
                created_at: Instant::now(),
                child_process: None,
            },
        );
        Ok(())
    }

    /// Build a reqwest client for SSE connections.
    fn build_sse_client(
        &self,
        headers: &reqwest::header::HeaderMap,
    ) -> Result<reqwest::Client, McpError> {
        reqwest::Client::builder()
            .default_headers(headers.clone())
            .build()
            .map_err(|e| McpError::ConnectionFailed(format!("Failed to build SSE client: {e}")))
    }

    // @zen-impl: PLAN-033 T-XMCP-043 — connect managed HTTP/SSE server
    /// Connect to a managed HTTP/SSE MCP server by spawning a child process
    /// and then connecting via the appropriate protocol.
    async fn connect_managed(
        &self,
        server: &crate::models::mcp::McpServerRow,
        server_id: Uuid,
        transport_type: TransportType,
    ) -> Result<(), McpError> {
        // Enforce managed process limit
        if self.managed_count() >= self.max_managed_processes && !self.evict_lru_managed() {
            return Err(McpError::ResourceExhausted(format!(
                "Maximum managed process limit ({}) reached",
                self.max_managed_processes
            )));
        }

        let config: ManagedHttpServerConfig = server
            .config
            .as_ref()
            .and_then(|c| {
                if let Ok(sc) = serde_json::from_value::<ServerConfig>(c.clone()) {
                    match sc {
                        ServerConfig::ManagedSse(m) | ServerConfig::ManagedHttp(m) => {
                            return Some(m);
                        }
                        _ => {}
                    }
                }
                serde_json::from_value(c.clone()).ok()
            })
            .ok_or_else(|| {
                McpError::Validation(format!(
                    "Server \"{}\" has no valid managed HTTP configuration",
                    server.name
                ))
            })?;

        // Spawn the child process
        let mut child = spawn_managed_process(&config).map_err(|e| {
            McpError::ConnectionFailed(format!(
                "Failed to spawn managed process '{}': {e}",
                config.command
            ))
        })?;

        // Write PID to terminator manifest
        if let Some(ref manifest) = self.manifest_path {
            if let Some(pid) = child.id() {
                if let Err(e) = append_manifest(manifest, pid) {
                    warn!("Failed to write managed PID {pid} to manifest: {e}");
                }
            }
        }

        // Determine the URL and path
        let default_path = match transport_type {
            TransportType::ManagedSse => "/sse",
            TransportType::ManagedHttp => "/mcp",
            _ => "/mcp",
        };
        let path = config.path.as_deref().unwrap_or(default_path);
        let url = format!("http://localhost:{}{}", config.port, path);

        // Wait for the server to become ready
        let timeout_secs = config.ready_timeout_secs.unwrap_or(30);
        let timeout = Duration::from_secs(timeout_secs as u64);
        wait_for_ready(&url, timeout).await.map_err(|e| {
            // Kill the child process on failure
            let _ = child.start_kill();
            McpError::ConnectionFailed(format!(
                "Managed server '{}' did not become ready within {timeout_secs}s: {e}",
                server.name
            ))
        })?;

        // Connect via the appropriate protocol
        let service: RunningService<RoleClient, ()> = match transport_type {
            TransportType::ManagedSse => {
                let transport = super::sse_transport::SseClientTransport::new(&url);
                tokio::time::timeout(STDIO_CONNECT_TIMEOUT, ().serve(transport))
                    .await
                    .map_err(|_| {
                        let _ = child.start_kill();
                        McpError::ConnectionFailed(format!(
                            "Managed SSE server '{}' did not respond within {}s",
                            server.name,
                            STDIO_CONNECT_TIMEOUT.as_secs()
                        ))
                    })?
                    .map_err(|e| {
                        let _ = child.start_kill();
                        McpError::ConnectionFailed(format!(
                            "Failed to initialize managed SSE server {}: {e}",
                            server.name
                        ))
                    })?
            }
            TransportType::ManagedHttp => {
                let config = StreamableHttpClientTransportConfig::with_uri(&*url);
                let transport = StreamableHttpClientTransport::from_config(config);
                tokio::time::timeout(STDIO_CONNECT_TIMEOUT, ().serve(transport))
                    .await
                    .map_err(|_| {
                        let _ = child.start_kill();
                        McpError::ConnectionFailed(format!(
                            "Managed HTTP server '{}' did not respond within {}s",
                            server.name,
                            STDIO_CONNECT_TIMEOUT.as_secs()
                        ))
                    })?
                    .map_err(|e| {
                        let _ = child.start_kill();
                        McpError::ConnectionFailed(format!(
                            "Failed to initialize managed HTTP server {}: {e}",
                            server.name
                        ))
                    })?
            }
            _ => unreachable!("connect_managed called with non-managed transport type"),
        };

        info!(
            server_name = %server.name,
            server_id = %server_id,
            transport = ?transport_type,
            "Managed MCP server connected"
        );

        self.connections.insert(
            server_id,
            PoolEntry {
                service,
                transport: transport_type,
                last_accessed: AtomicU64::new(self.epoch.elapsed().as_millis() as u64),
                created_at: Instant::now(),
                child_process: Some(child),
            },
        );
        Ok(())
    }

    // @zen-impl: PLAN-033 T-XMCP-062 — kill child process on removal
    /// Remove a stale connection, killing any child process.
    fn remove(&self, server_id: &Uuid) {
        if let Some((_, mut entry)) = self.connections.remove(server_id) {
            let ct = entry.service.cancellation_token();
            ct.cancel();
            if let Some(ref mut child) = entry.child_process {
                let _ = child.start_kill();
            }
        }
    }

    // @zen-impl: PLAN-030 Phase 2.1 — evict idle managed connections
    // @zen-impl: PLAN-033 T-XMCP-052 — evict all managed transports, not just stdio
    /// Evict all managed connections that have been idle longer than `timeout`.
    fn evict_idle(&self, timeout: Duration) {
        let mut evicted = Vec::new();
        self.connections.retain(|id, entry| {
            if entry.transport.is_managed() && entry.idle_duration(&self.epoch) > timeout {
                evicted.push(*id);
                if let Some(ref mut child) = entry.child_process {
                    let _ = child.start_kill();
                }
                false
            } else {
                true
            }
        });
        for id in &evicted {
            info!(server_id = %id, "Evicted idle managed connection");
        }
    }

    // @zen-impl: PLAN-030 Phase 2.2 — spawn background reaper
    /// Spawn a background reaper task that evicts idle managed connections.
    /// Returns a `JoinHandle` — the task runs until the Tokio runtime shuts down.
    pub fn spawn_reaper(self: &Arc<Self>, idle_timeout: Duration) -> tokio::task::JoinHandle<()> {
        let pool = Arc::clone(self);
        let interval = idle_timeout / 4;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                pool.evict_idle(idle_timeout);
            }
        })
    }

    // @zen-impl: PLAN-030 Phase 3.1 — LRU eviction for capacity management
    // @zen-impl: PLAN-033 T-XMCP-052 — evict LRU across all managed transports
    /// Evict the single least-recently-used managed connection.
    /// Returns `true` if an entry was evicted.
    fn evict_lru_managed(&self) -> bool {
        let oldest = self
            .connections
            .iter()
            .filter(|e| e.value().transport.is_managed())
            .min_by_key(|e| e.value().last_accessed.load(Ordering::Relaxed))
            .map(|e| *e.key());

        if let Some(id) = oldest {
            self.remove(&id);
            info!(server_id = %id, "LRU-evicted managed connection to make room");
            true
        } else {
            false
        }
    }
}

impl Default for ClientPool {
    fn default() -> Self {
        Self::new()
    }
}

// @zen-impl: PLAN-025 Phase 5.2 — append PID kill command to terminator manifest
/// Appends a `kill <pid>` line to the terminator manifest file (atomic append + fsync).
fn append_manifest(manifest: &Path, pid: u32) -> Result<(), String> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(manifest)
        .map_err(|e| format!("open manifest for append: {e}"))?;

    writeln!(file, "kill {pid}").map_err(|e| format!("write to manifest: {e}"))?;
    file.flush().map_err(|e| format!("flush manifest: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("fsync manifest: {e}"))?;

    Ok(())
}

// =============================================================================
// Managed process helpers
// =============================================================================

// @zen-impl: PLAN-033 T-XMCP-041 — spawn managed child process
/// Spawn a managed child process with piped stdin for lifecycle coupling.
fn spawn_managed_process(
    config: &ManagedHttpServerConfig,
) -> Result<tokio::process::Child, String> {
    let args = config.args.as_deref().unwrap_or_default();

    let mut cmd = tokio::process::Command::new(&config.command);
    cmd.args(args)
        .stdin(std::process::Stdio::piped()) // lifecycle coupling
        .stderr(std::process::Stdio::inherit());

    // Set environment variables if provided
    if let Some(env) = &config.env {
        for (k, v) in env {
            cmd.env(k, v);
        }
    }

    cmd.spawn()
        .map_err(|e| format!("spawn '{}': {e}", config.command))
}

// @zen-impl: PLAN-033 T-XMCP-042 — wait for managed server readiness
/// Retry HTTP GET to the given URL until it succeeds or timeout elapses.
async fn wait_for_ready(url: &str, timeout: Duration) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| format!("build readiness client: {e}"))?;

    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        // Try a simple GET — we just need the server to accept connections
        match client.get(url).send().await {
            Ok(_) => return Ok(()),
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(READY_RETRY_INTERVAL).await;
            }
            Err(e) => {
                return Err(format!(
                    "server not ready after {}s: {e}",
                    timeout.as_secs()
                ));
            }
        }
    }
}

// =============================================================================
// HTTP connection testing via rmcp
// =============================================================================

/// Test an HTTP MCP server connection using the same rmcp transport as real
/// tool execution. Performs a full `initialize` handshake and `tools/list`,
/// handling session IDs and protocol negotiation correctly.
pub async fn test_http_connection(
    config: &HttpServerConfig,
    api_key: Option<&str>,
    oauth_token: Option<&str>,
) -> TestConnectionResult {
    let server_url = &config.url;
    if server_url.is_empty() {
        return TestConnectionResult {
            success: false,
            error: Some("URL is required for HTTP transport".into()),
            ..Default::default()
        };
    }

    // Build the rmcp StreamableHttp transport (same path as connect_http)
    let mut transport_config = StreamableHttpClientTransportConfig::with_uri(server_url.as_str());

    let mut header_map = reqwest::header::HeaderMap::new();

    // Configure auth
    if config.auth_type == "api-key" {
        if let Some(key) = api_key {
            let header_name = config.api_key_header.as_deref().unwrap_or("X-API-Key");
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(header_name.as_bytes()),
                reqwest::header::HeaderValue::from_str(key),
            ) {
                header_map.insert(name, val);
            }
        }
    } else if config.auth_type == "oauth"
        && let Some(token) = oauth_token
    {
        transport_config.auth_header = Some(token.to_string());
    }

    // Add custom headers from config
    add_custom_headers(&mut header_map, &config.headers);

    let client = match reqwest::Client::builder()
        .default_headers(header_map)
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return TestConnectionResult {
                success: false,
                error: Some(format!("Failed to create HTTP client: {e}")),
                ..Default::default()
            };
        }
    };
    let transport = StreamableHttpClientTransport::with_client(client, transport_config);

    // Connect via rmcp (performs initialize handshake + sends initialized notification)
    let service: RunningService<RoleClient, ()> = match ().serve(transport).await {
        Ok(s) => s,
        Err(e) => {
            let error = format!("{e}");
            let (error_msg, error_details) = if error.contains("timed out") {
                ("Connection timed out (15s)".to_string(), None)
            } else if error.contains("refused") {
                ("Connection refused".to_string(), None)
            } else {
                ("Connection failed".to_string(), Some(error))
            };
            return TestConnectionResult {
                success: false,
                error: Some(error_msg),
                error_details,
                ..Default::default()
            };
        }
    };

    // Extract server info from the initialize result
    let (server_name, server_version, protocol_version) = match service.peer_info() {
        Some(info) => (
            Some(info.server_info.name.clone()),
            Some(info.server_info.version.clone()),
            Some(info.protocol_version.to_string()),
        ),
        None => (None, None, None),
    };

    // List all tools (handles pagination automatically)
    let tools: Vec<McpToolSummary> = match service.peer().list_all_tools().await {
        Ok(rmcp_tools) => rmcp_tools
            .into_iter()
            .map(|t| McpToolSummary {
                name: t.name.to_string(),
                description: t.description.as_deref().unwrap_or("").to_string(),
            })
            .collect(),
        Err(e) => {
            warn!("tools/list failed during connection test: {e}");
            vec![]
        }
    };

    let tool_count = tools.len() as i64;

    TestConnectionResult {
        success: true,
        server_name,
        server_version,
        protocol_version,
        tool_count: Some(tool_count),
        error: None,
        error_details: None,
        auth_required: None,
        tools,
    }
}

// =============================================================================
// Stdio connection testing via rmcp
// =============================================================================

/// Test a stdio MCP server connection by spawning the process and performing
/// an MCP handshake via rmcp's `TokioChildProcess` transport.
///
/// Returns server info and discovered tools on success.
pub async fn test_stdio_connection(config: &StdioServerConfig) -> TestConnectionResult {
    if config.command.is_empty() {
        return TestConnectionResult {
            success: false,
            error: Some("Command is required for stdio transport".into()),
            ..Default::default()
        };
    }

    let args = config.args.as_deref().unwrap_or_default();

    let mut cmd = tokio::process::Command::new(&config.command);
    cmd.args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Set environment variables if provided
    if let Some(env) = &config.env {
        for (k, v) in env {
            cmd.env(k, v);
        }
    }

    let transport = match TokioChildProcess::new(cmd) {
        Ok(t) => t,
        Err(e) => {
            return TestConnectionResult {
                success: false,
                error: Some(format!("Failed to spawn process '{}': {e}", config.command)),
                ..Default::default()
            };
        }
    };

    // Connect via rmcp with a timeout (performs initialize + initialized notification)
    let service: RunningService<RoleClient, ()> =
        match tokio::time::timeout(STDIO_CONNECT_TIMEOUT, ().serve(transport)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                let error = format!("{e}");
                return TestConnectionResult {
                    success: false,
                    error: Some("Stdio communication error".to_string()),
                    error_details: Some(error),
                    ..Default::default()
                };
            }
            Err(_) => {
                return TestConnectionResult {
                    success: false,
                    error: Some(format!(
                        "Connection timed out ({}s)",
                        STDIO_CONNECT_TIMEOUT.as_secs()
                    )),
                    ..Default::default()
                };
            }
        };

    // Extract server info from the initialize result
    let (server_name, server_version, protocol_version) = match service.peer_info() {
        Some(info) => (
            Some(info.server_info.name.clone()),
            Some(info.server_info.version.clone()),
            Some(info.protocol_version.to_string()),
        ),
        None => (None, None, None),
    };

    // List all tools (handles pagination automatically)
    let tools: Vec<McpToolSummary> = match service.peer().list_all_tools().await {
        Ok(rmcp_tools) => rmcp_tools
            .into_iter()
            .map(|t| McpToolSummary {
                name: t.name.to_string(),
                description: t.description.as_deref().unwrap_or("").to_string(),
            })
            .collect(),
        Err(e) => {
            warn!("tools/list failed during stdio connection test: {e}");
            vec![]
        }
    };

    let tool_count = tools.len() as i64;

    TestConnectionResult {
        success: true,
        server_name,
        server_version,
        protocol_version,
        tool_count: Some(tool_count),
        error: None,
        error_details: None,
        auth_required: None,
        tools,
    }
}

// =============================================================================
// SSE connection testing via rmcp
// =============================================================================

// @zen-impl: PLAN-033 T-XMCP-070 — test SSE connection
/// Test a legacy SSE MCP server connection by connecting to the SSE endpoint
/// and performing an MCP handshake.
pub async fn test_sse_connection(
    config: &SseServerConfig,
    api_key: Option<&str>,
    oauth_token: Option<&str>,
) -> TestConnectionResult {
    if config.url.is_empty() {
        return TestConnectionResult {
            success: false,
            error: Some("URL is required for SSE transport".into()),
            ..Default::default()
        };
    }

    let mut extra_headers = reqwest::header::HeaderMap::new();

    // Configure auth
    if config.auth_type == "api-key" {
        if let Some(key) = api_key {
            let header_name = config.api_key_header.as_deref().unwrap_or("X-API-Key");
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(header_name.as_bytes()),
                reqwest::header::HeaderValue::from_str(key),
            ) {
                extra_headers.insert(name, val);
            }
        }
    } else if config.auth_type == "oauth" {
        if let Some(token) = oauth_token {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")) {
                extra_headers.insert(reqwest::header::AUTHORIZATION, val);
            }
        }
    }

    // Add custom headers from config
    add_custom_headers(&mut extra_headers, &config.headers);

    let client = match reqwest::Client::builder()
        .default_headers(extra_headers.clone())
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return TestConnectionResult {
                success: false,
                error: Some(format!("Failed to create SSE client: {e}")),
                ..Default::default()
            };
        }
    };

    let transport =
        super::sse_transport::SseClientTransport::with_client(client, &config.url, extra_headers);

    // Connect via rmcp (performs initialize handshake)
    let service: RunningService<RoleClient, ()> = match ().serve(transport).await {
        Ok(s) => s,
        Err(e) => {
            let error = format!("{e}");
            let (error_msg, error_details) = if error.contains("timed out") {
                ("Connection timed out (15s)".to_string(), None)
            } else if error.contains("refused") {
                ("Connection refused".to_string(), None)
            } else {
                ("Connection failed".to_string(), Some(error))
            };
            return TestConnectionResult {
                success: false,
                error: Some(error_msg),
                error_details,
                ..Default::default()
            };
        }
    };

    // Extract server info
    let (server_name, server_version, protocol_version) = match service.peer_info() {
        Some(info) => (
            Some(info.server_info.name.clone()),
            Some(info.server_info.version.clone()),
            Some(info.protocol_version.to_string()),
        ),
        None => (None, None, None),
    };

    // List all tools
    let tools: Vec<McpToolSummary> = match service.peer().list_all_tools().await {
        Ok(rmcp_tools) => rmcp_tools
            .into_iter()
            .map(|t| McpToolSummary {
                name: t.name.to_string(),
                description: t.description.as_deref().unwrap_or("").to_string(),
            })
            .collect(),
        Err(e) => {
            warn!("tools/list failed during SSE connection test: {e}");
            vec![]
        }
    };

    let tool_count = tools.len() as i64;

    TestConnectionResult {
        success: true,
        server_name,
        server_version,
        protocol_version,
        tool_count: Some(tool_count),
        error: None,
        error_details: None,
        auth_required: None,
        tools,
    }
}

// =============================================================================
// Managed connection testing (spawn → connect → test → kill)
// =============================================================================

// @zen-impl: PLAN-033 T-XMCP-072 — test managed transport connection
/// Test a managed MCP server by spawning a temporary child process, waiting for
/// it to become ready, connecting via the appropriate protocol (SSE or
/// StreamableHttp), then performing an MCP handshake and tool discovery.
/// The child process is killed after the test.
pub async fn test_managed_connection(
    config: &ManagedHttpServerConfig,
    transport_type: &TransportType,
) -> TestConnectionResult {
    if config.command.is_empty() {
        return TestConnectionResult {
            success: false,
            error: Some("Command is required for managed transport".into()),
            ..Default::default()
        };
    }
    if config.port == 0 {
        return TestConnectionResult {
            success: false,
            error: Some("Port is required for managed transport".into()),
            ..Default::default()
        };
    }

    // Spawn temporary child process
    let mut child = match spawn_managed_process(config) {
        Ok(c) => c,
        Err(e) => {
            return TestConnectionResult {
                success: false,
                error: Some(format!("Failed to spawn process: {e}")),
                ..Default::default()
            };
        }
    };

    let path = config.path.as_deref().unwrap_or("/sse");
    let url = format!("http://127.0.0.1:{}{path}", config.port);
    let ready_timeout = Duration::from_secs(config.ready_timeout_secs.unwrap_or(30) as u64);

    // Wait for the server to become ready
    if let Err(e) = wait_for_ready(&url, ready_timeout).await {
        let _ = child.start_kill();
        return TestConnectionResult {
            success: false,
            error: Some(format!("Server not ready: {e}")),
            ..Default::default()
        };
    }

    // Connect via the appropriate protocol
    let result = match transport_type {
        TransportType::ManagedSse => {
            let transport = super::sse_transport::SseClientTransport::new(&url);
            match tokio::time::timeout(STDIO_CONNECT_TIMEOUT, ().serve(transport)).await {
                Ok(Ok(service)) => extract_test_result(&service, "managed SSE").await,
                Ok(Err(e)) => TestConnectionResult {
                    success: false,
                    error: Some("Managed SSE communication error".to_string()),
                    error_details: Some(format!("{e}")),
                    ..Default::default()
                },
                Err(_) => TestConnectionResult {
                    success: false,
                    error: Some(format!(
                        "Managed SSE connection timed out ({}s)",
                        STDIO_CONNECT_TIMEOUT.as_secs()
                    )),
                    ..Default::default()
                },
            }
        }
        TransportType::ManagedHttp => {
            let http_path = config.path.as_deref().unwrap_or("/mcp");
            let http_url = format!("http://127.0.0.1:{}{http_path}", config.port);
            let cfg = StreamableHttpClientTransportConfig::with_uri(&*http_url);
            let transport = StreamableHttpClientTransport::from_config(cfg);
            match tokio::time::timeout(STDIO_CONNECT_TIMEOUT, ().serve(transport)).await {
                Ok(Ok(service)) => extract_test_result(&service, "managed HTTP").await,
                Ok(Err(e)) => TestConnectionResult {
                    success: false,
                    error: Some("Managed HTTP communication error".to_string()),
                    error_details: Some(format!("{e}")),
                    ..Default::default()
                },
                Err(_) => TestConnectionResult {
                    success: false,
                    error: Some(format!(
                        "Managed HTTP connection timed out ({}s)",
                        STDIO_CONNECT_TIMEOUT.as_secs()
                    )),
                    ..Default::default()
                },
            }
        }
        _ => TestConnectionResult {
            success: false,
            error: Some(format!(
                "Unexpected transport type for managed test: {transport_type:?}"
            )),
            ..Default::default()
        },
    };

    // Kill the temporary child process
    let _ = child.start_kill();

    result
}

/// Extract server info and tools from a connected RunningService (shared by test functions).
async fn extract_test_result(
    service: &RunningService<RoleClient, ()>,
    label: &str,
) -> TestConnectionResult {
    let (server_name, server_version, protocol_version) = match service.peer_info() {
        Some(info) => (
            Some(info.server_info.name.clone()),
            Some(info.server_info.version.clone()),
            Some(info.protocol_version.to_string()),
        ),
        None => (None, None, None),
    };

    let tools: Vec<McpToolSummary> = match service.peer().list_all_tools().await {
        Ok(rmcp_tools) => rmcp_tools
            .into_iter()
            .map(|t| McpToolSummary {
                name: t.name.to_string(),
                description: t.description.as_deref().unwrap_or("").to_string(),
            })
            .collect(),
        Err(e) => {
            warn!("tools/list failed during {label} connection test: {e}");
            vec![]
        }
    };

    let tool_count = tools.len() as i64;

    TestConnectionResult {
        success: true,
        server_name,
        server_version,
        protocol_version,
        tool_count: Some(tool_count),
        error: None,
        error_details: None,
        auth_required: None,
        tools,
    }
}

/// Add custom headers from JSON config to a reqwest HeaderMap.
fn add_custom_headers(
    header_map: &mut reqwest::header::HeaderMap,
    headers: &Option<serde_json::Value>,
) {
    if let Some(hdrs) = headers
        && let Some(map) = hdrs.as_object()
    {
        for (k, v) in map {
            if let Some(val) = v.as_str()
                && let (Ok(name), Ok(value)) = (
                    reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                    reqwest::header::HeaderValue::from_str(val),
                )
            {
                header_map.insert(name, value);
            }
        }
    }
}

/// Resolve OAuth headers for a server, refreshing tokens if needed.
/// Returns `None` if the server does not use OAuth auth.
// @zen-impl: PLAN-031 Phase 7.3 — token refresh before connection
async fn resolve_oauth_headers(
    pool: &PgPool,
    user_id: &str,
    server_id: Uuid,
    encryption_key: &str,
) -> Result<Option<OAuthHeaders>, McpError> {
    let server = queries::get_server(pool, &server_id.to_string())
        .await?
        .ok_or_else(|| McpError::NotFound(format!("Server {server_id}")))?;

    let auth_type = queries::extract_auth_type(&server.config);
    if auth_type != AuthType::OAuth {
        return Ok(None);
    }

    // Load token row
    let token_row = queries::get_oauth_token(pool, user_id, &server_id.to_string())
        .await?
        .ok_or_else(|| {
            McpError::ConnectionFailed(
                "No OAuth tokens found — please authorize this server first".into(),
            )
        })?;

    // Check if tokens need refresh
    let needs_refresh = super::oauth::should_refresh(&token_row.expires_at);

    if needs_refresh {
        // Load OAuth config and client secret for refresh
        let oauth_config_json = server.oauth_config.ok_or_else(|| {
            McpError::ConnectionFailed("Server missing OAuth configuration".into())
        })?;
        let oauth_config: crate::models::mcp::OAuthConfig =
            serde_json::from_value(oauth_config_json)
                .map_err(|e| McpError::ConnectionFailed(format!("Invalid OAuth config: {e}")))?;

        let encrypted_secret =
            queries::get_oauth_client_secret_encrypted(pool, &server_id.to_string())
                .await?
                .ok_or_else(|| {
                    McpError::ConnectionFailed("No OAuth client secret stored".into())
                })?;
        let client_secret = super::secrets::decrypt(&encrypted_secret, encryption_key)?;

        let refresh_token_encrypted =
            token_row
                .refresh_token_encrypted
                .as_deref()
                .ok_or_else(|| {
                    McpError::ConnectionFailed(
                        "No refresh token — please re-authorize this server".into(),
                    )
                })?;
        let refresh_token = super::secrets::decrypt(refresh_token_encrypted, encryption_key)?;

        // Refresh tokens
        let resp = super::oauth::refresh_google_tokens(
            &oauth_config.token_url,
            &oauth_config.client_id,
            &client_secret,
            &refresh_token,
        )
        .await?;

        // Encrypt and store refreshed tokens
        let id_token_encrypted = match &resp.id_token {
            Some(t) => Some(super::secrets::encrypt(t, encryption_key)?),
            None => None,
        };
        let access_token_encrypted = super::secrets::encrypt(&resp.access_token, encryption_key)?;
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(resp.expires_in);
        let scope_vec: Vec<String> = resp
            .scope
            .as_deref()
            .unwrap_or("")
            .split_whitespace()
            .map(String::from)
            .collect();

        queries::store_oauth_token(
            pool,
            user_id,
            &server_id.to_string(),
            id_token_encrypted.as_deref(),
            &access_token_encrypted,
            None, // refresh_token unchanged, COALESCE preserves it
            expires_at,
            &scope_vec,
        )
        .await?;

        // Use the freshly refreshed tokens
        let id_token = resp
            .id_token
            .ok_or_else(|| McpError::ConnectionFailed("No id_token in refresh response".into()))?;

        return Ok(Some(OAuthHeaders {
            id_token,
            access_token: resp.access_token,
        }));
    }

    // Tokens are still valid — decrypt and return
    let id_token_encrypted = token_row.id_token_encrypted.as_deref().ok_or_else(|| {
        McpError::ConnectionFailed("No id_token stored — please re-authorize".into())
    })?;
    let id_token = super::secrets::decrypt(id_token_encrypted, encryption_key)?;

    let access_token_encrypted = &token_row.access_token_encrypted;
    let access_token = super::secrets::decrypt(access_token_encrypted, encryption_key)?;

    Ok(Some(OAuthHeaders {
        id_token,
        access_token,
    }))
}

/// Execute a tool on an external MCP server.
///
/// 1. Validates the tool exists and the user has access.
/// 2. Connects to the external server (or reuses a pooled connection).
/// 3. Calls the tool with the provided parameters.
/// 4. Records an audit log entry.
/// 5. Returns the result.
// @zen-impl: PLAN-031 Phase 7.3 — OAuth token lifecycle during tool execution
pub async fn execute_tool(
    pool: &PgPool,
    client_pool: &ClientPool,
    request: &ExecutionRequest,
    encryption_key: &str,
) -> Result<ExecutionResult, McpError> {
    // Validate tool exists and user has access
    let tool = queries::get_tool_manifest(pool, &request.user_id, &request.tool_id.to_string())
        .await?
        .ok_or_else(|| {
            McpError::NotFound(format!(
                "Tool {} not found or access denied",
                request.tool_id
            ))
        })?;

    let server_id = tool.server_id;

    // Resolve OAuth headers if the server uses OAuth auth
    let oauth_headers =
        resolve_oauth_headers(pool, &request.user_id, server_id, encryption_key).await?;

    // Convert params to JsonObject
    let arguments = request.params.clone();

    // Build call params
    let call_params = CallToolRequestParams {
        meta: None,
        name: Cow::Owned(request.tool_name.clone()),
        arguments,
        task: None,
    };

    // Try to execute with one retry on connection error
    let result = execute_with_retry(
        pool,
        client_pool,
        server_id,
        &call_params,
        oauth_headers.as_ref(),
    )
    .await?;

    // Record audit log (fire-and-forget)
    let is_error = result.is_error.unwrap_or(false);
    let audit_details = serde_json::json!({
        "toolId": request.tool_id.to_string(),
        "toolName": request.tool_name,
        "success": !is_error,
    });

    let server = queries::get_server(pool, &server_id.to_string()).await?;
    let server_name = server
        .as_ref()
        .map(|s| s.name.as_str())
        .unwrap_or("unknown");

    if let Err(e) = queries::insert_audit_log(
        pool,
        &request.user_id,
        Some(&server_id.to_string()),
        server_name,
        "tool_execution",
        Some(&audit_details),
    )
    .await
    {
        warn!("Failed to record audit log: {e}");
    }

    // Convert CallToolResult to our ExecutionResult
    let result_json = call_tool_result_to_json(&result);

    Ok(ExecutionResult {
        success: !is_error,
        tool_name: request.tool_name.clone(),
        result: result_json,
    })
}

/// Execute a tool call with one retry on connection error.
async fn execute_with_retry(
    pool: &PgPool,
    client_pool: &ClientPool,
    server_id: Uuid,
    params: &CallToolRequestParams,
    oauth_headers: Option<&OAuthHeaders>,
) -> Result<CallToolResult, McpError> {
    // Attempt 1
    client_pool
        .get_or_connect(pool, server_id, oauth_headers)
        .await?;

    match call_tool(client_pool, server_id, params).await {
        Ok(result) => return Ok(result),
        Err(e) => {
            debug!("Tool call failed, retrying after reconnect: {e}");
            client_pool.remove(&server_id);
        }
    }

    // Attempt 2 (reconnect)
    client_pool
        .get_or_connect(pool, server_id, oauth_headers)
        .await?;
    call_tool(client_pool, server_id, params).await
}

/// Call a tool on a connected MCP server with timeout.
async fn call_tool(
    client_pool: &ClientPool,
    server_id: Uuid,
    params: &CallToolRequestParams,
) -> Result<CallToolResult, McpError> {
    let conn = client_pool
        .connections
        .get(&server_id)
        .ok_or_else(|| McpError::ConnectionFailed("No connection available".into()))?;

    conn.touch(&client_pool.epoch);
    let peer = conn.service.peer().clone();
    drop(conn); // Release the DashMap ref before awaiting

    let result = tokio::time::timeout(DEFAULT_TIMEOUT, peer.call_tool(params.clone()))
        .await
        .map_err(|_| McpError::ConnectionFailed("Tool execution timed out (30s)".into()))?
        .map_err(|e| McpError::ConnectionFailed(format!("Tool call failed: {e}")))?;

    Ok(result)
}

/// Convert a `CallToolResult` to a JSON value for our response.
fn call_tool_result_to_json(result: &CallToolResult) -> serde_json::Value {
    use rmcp::model::RawContent;

    let content_values: Vec<serde_json::Value> = result
        .content
        .iter()
        .map(|c| match &c.raw {
            RawContent::Text(text) => serde_json::json!({
                "type": "text",
                "text": text.text,
            }),
            RawContent::Image(img) => serde_json::json!({
                "type": "image",
                "data": img.data,
                "mimeType": img.mime_type,
            }),
            RawContent::Audio(audio) => serde_json::json!({
                "type": "audio",
                "data": audio.data,
                "mimeType": audio.mime_type,
            }),
            RawContent::Resource(res) => serde_json::json!({
                "type": "resource",
                "resource": res.resource,
            }),
            RawContent::ResourceLink(link) => serde_json::json!({
                "type": "resource_link",
                "uri": link.uri,
                "name": link.name,
            }),
        })
        .collect();

    serde_json::json!({
        "content": content_values,
        "isError": result.is_error.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // @zen-test: PLAN-025 Phase 2 — ClientPool construction
    #[test]
    fn client_pool_new_creates_empty_pool() {
        let pool = ClientPool::new();
        assert_eq!(pool.connections.len(), 0);
        assert!(pool.manifest_path.is_none());
        assert_eq!(pool.max_managed_processes, DEFAULT_MAX_MANAGED_PROCESSES);
    }

    // @zen-test: PLAN-025 Phase 5.4 — ClientPool with manifest
    #[test]
    fn client_pool_with_manifest_stores_path() {
        let path = PathBuf::from("/tmp/test-manifest");
        let pool = ClientPool::with_manifest(path.clone());
        assert_eq!(pool.manifest_path, Some(path));
    }

    // @zen-test: PLAN-025 Phase 2.3 — max managed processes configuration
    #[test]
    fn client_pool_set_max_managed_processes() {
        let mut pool = ClientPool::new();
        pool.set_max_managed_processes(10);
        assert_eq!(pool.max_managed_processes, 10);
    }

    // @zen-test: PLAN-025 Phase 2.3 — managed count tracking
    #[test]
    fn client_pool_managed_count_tracks_transport_type() {
        let pool = ClientPool::new();
        assert_eq!(pool.managed_count(), 0);
    }

    // @zen-test: PLAN-025 Phase 2 — Default implementation
    #[test]
    fn client_pool_default_matches_new() {
        let pool = ClientPool::default();
        assert_eq!(pool.connections.len(), 0);
        assert!(pool.manifest_path.is_none());
        assert_eq!(pool.max_managed_processes, DEFAULT_MAX_MANAGED_PROCESSES);
    }

    // @zen-test: PLAN-025 Phase 5.2 — manifest PID append
    #[test]
    fn append_manifest_writes_kill_command() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("manifest.txt");
        std::fs::write(&manifest, "").unwrap();

        append_manifest(&manifest, 12345).unwrap();

        let content = std::fs::read_to_string(&manifest).unwrap();
        assert_eq!(content, "kill 12345\n");
    }

    // @zen-test: PLAN-025 Phase 5.2 — manifest appends multiple PIDs
    #[test]
    fn append_manifest_appends_multiple_pids() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("manifest.txt");
        std::fs::write(&manifest, "kill 100\n").unwrap();

        append_manifest(&manifest, 200).unwrap();
        append_manifest(&manifest, 300).unwrap();

        let content = std::fs::read_to_string(&manifest).unwrap();
        assert_eq!(content, "kill 100\nkill 200\nkill 300\n");
    }

    // @zen-test: PLAN-025 Phase 5.2 — manifest append fails for missing file
    #[test]
    fn append_manifest_fails_for_missing_file() {
        let result = append_manifest(Path::new("/nonexistent/manifest.txt"), 123);
        assert!(result.is_err());
    }

    // @zen-test: PLAN-025 Phase 2 — remove on empty pool is no-op
    #[test]
    fn client_pool_remove_nonexistent_is_noop() {
        let pool = ClientPool::new();
        pool.remove(&Uuid::new_v4()); // Should not panic
    }

    // @zen-test: PLAN-030 Phase 1.2 — last_accessed AtomicU64 stores and loads correctly
    #[test]
    fn pool_entry_last_accessed_atomic_roundtrip() {
        let epoch = Instant::now();
        let last_accessed = AtomicU64::new(0);

        // Initially zero
        assert_eq!(last_accessed.load(Ordering::Relaxed), 0);

        // Simulate touch
        std::thread::sleep(Duration::from_millis(5));
        let ms = epoch.elapsed().as_millis() as u64;
        last_accessed.store(ms, Ordering::Relaxed);

        let stored = last_accessed.load(Ordering::Relaxed);
        assert!(stored >= 5, "expected at least 5ms, got {stored}");
    }

    // @zen-test: PLAN-030 Phase 1.2 — idle_duration increases over time
    #[test]
    fn pool_entry_idle_duration_logic() {
        let epoch = Instant::now();
        let last_accessed = AtomicU64::new(epoch.elapsed().as_millis() as u64);

        // Simulate time passing
        std::thread::sleep(Duration::from_millis(10));

        let now_ms = epoch.elapsed().as_millis() as u64;
        let last = last_accessed.load(Ordering::Relaxed);
        let idle = Duration::from_millis(now_ms.saturating_sub(last));

        assert!(idle >= Duration::from_millis(10));
    }

    // @zen-test: PLAN-030 Phase 1.3 — idle_timeout configuration
    #[test]
    fn client_pool_set_idle_timeout() {
        let mut pool = ClientPool::new();
        assert_eq!(pool.idle_timeout(), DEFAULT_IDLE_TIMEOUT);
        pool.set_idle_timeout(Duration::from_secs(60));
        assert_eq!(pool.idle_timeout(), Duration::from_secs(60));
    }

    // @zen-test: PLAN-030 Phase 1.1 — epoch is initialized
    #[test]
    fn client_pool_epoch_is_recent() {
        let before = Instant::now();
        let pool = ClientPool::new();
        let after = Instant::now();
        // epoch should be between before and after
        assert!(pool.epoch >= before);
        assert!(pool.epoch <= after);
    }

    // @zen-test: PLAN-030 Phase 3.1 — evict_lru_managed returns false on empty pool
    #[test]
    fn evict_lru_managed_returns_false_on_empty_pool() {
        let pool = ClientPool::new();
        assert!(!pool.evict_lru_managed());
    }

    // @zen-test: PLAN-030 Phase 2.1 — evict_idle is no-op on empty pool
    #[test]
    fn evict_idle_noop_on_empty_pool() {
        let pool = ClientPool::new();
        pool.evict_idle(Duration::from_secs(1)); // Should not panic
        assert_eq!(pool.connections.len(), 0);
    }

    // @zen-test: PLAN-030 Phase 1.1 — default pool has idle_timeout and epoch
    #[test]
    fn client_pool_default_has_idle_timeout_and_epoch() {
        let pool = ClientPool::default();
        assert_eq!(pool.idle_timeout(), DEFAULT_IDLE_TIMEOUT);
        // epoch should be roughly now
        assert!(pool.epoch.elapsed() < Duration::from_secs(1));
    }
}
