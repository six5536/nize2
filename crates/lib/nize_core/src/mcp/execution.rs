// @zen-component: MCP-ExecutionProxy
//
//! Execution proxy — connects to external MCP servers and executes tools.
//!
//! Maintains a pool of `RunningService` connections keyed by server ID.
//! Supports both HTTP (Streamable HTTP) and stdio (child process) transports.

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

use crate::models::mcp::{ServerConfig, StdioServerConfig, TransportType};

use super::McpError;
use super::queries;

/// Default timeout for tool execution (30 seconds).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default timeout for stdio server connection/initialization (30 seconds).
const STDIO_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default maximum number of concurrent stdio processes.
const DEFAULT_MAX_STDIO_PROCESSES: usize = 50;

/// Default idle timeout for stdio connections (5 minutes).
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

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
/// Supports both HTTP and stdio transports.
pub struct ClientPool {
    connections: Arc<DashMap<Uuid, PoolEntry>>,
    /// Guard set to prevent duplicate concurrent spawns for the same server.
    connecting: Arc<Mutex<HashSet<Uuid>>>,
    /// Path to the terminator manifest file for stdio process PID registration.
    manifest_path: Option<PathBuf>,
    /// Maximum number of concurrent stdio processes.
    max_stdio_processes: usize,
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
            max_stdio_processes: DEFAULT_MAX_STDIO_PROCESSES,
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

    /// Set the maximum number of concurrent stdio processes.
    pub fn set_max_stdio_processes(&mut self, max: usize) {
        self.max_stdio_processes = max;
    }

    /// Set the idle timeout for stdio connections.
    pub fn set_idle_timeout(&mut self, timeout: Duration) {
        self.idle_timeout = timeout;
    }

    /// Get the current idle timeout.
    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// Count current stdio connections.
    fn stdio_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|entry| entry.value().transport == TransportType::Stdio)
            .count()
    }

    // @zen-impl: PLAN-025 Phase 2.1 — atomic DashMap entry with connecting guard
    /// Get or create a connection to an MCP server.
    async fn get_or_connect(&self, pool: &PgPool, server_id: Uuid) -> Result<(), McpError> {
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

        let result = self.connect(pool, server_id).await;

        // Always remove from connecting guard.
        {
            let mut guard = self.connecting.lock().await;
            guard.remove(&server_id);
        }

        result
    }

    /// Internal connect logic — called within the connecting guard.
    async fn connect(&self, pool: &PgPool, server_id: Uuid) -> Result<(), McpError> {
        let server = queries::get_server(pool, &server_id.to_string())
            .await?
            .ok_or_else(|| McpError::NotFound(format!("Server {server_id}")))?;

        let transport_type = server.transport.clone();

        // @zen-impl: PLAN-025 Phase 2.2 — match on transport type
        match transport_type {
            TransportType::Http => self.connect_http(&server).await?,
            TransportType::Stdio => self.connect_stdio(&server, server_id).await?,
        }

        Ok(())
    }

    /// Connect to an HTTP MCP server via Streamable HTTP transport.
    async fn connect_http(
        &self,
        server: &crate::models::mcp::McpServerRow,
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

        // Build transport config with the HTTP URL
        let config = StreamableHttpClientTransportConfig::with_uri(&*url);
        let transport = StreamableHttpClientTransport::from_config(config);

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
        if self.stdio_count() >= self.max_stdio_processes && !self.evict_lru_stdio() {
            return Err(McpError::ResourceExhausted(format!(
                "Maximum stdio process limit ({}) reached",
                self.max_stdio_processes
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
        if let Some(ref manifest) = self.manifest_path {
            if let Some(pid) = transport.id() {
                if let Err(e) = append_manifest(manifest, pid) {
                    warn!("Failed to write stdio PID {pid} to manifest: {e}");
                }
            }
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
            },
        );
        Ok(())
    }

    /// Remove a stale connection.
    fn remove(&self, server_id: &Uuid) {
        if let Some((_, entry)) = self.connections.remove(server_id) {
            let ct = entry.service.cancellation_token();
            ct.cancel();
        }
    }

    // @zen-impl: PLAN-030 Phase 2.1 — evict idle stdio connections
    /// Evict all stdio connections that have been idle longer than `timeout`.
    fn evict_idle(&self, timeout: Duration) {
        let mut evicted = Vec::new();
        self.connections.retain(|id, entry| {
            if entry.transport == TransportType::Stdio && entry.idle_duration(&self.epoch) > timeout
            {
                evicted.push(*id);
                false
            } else {
                true
            }
        });
        for id in &evicted {
            info!(server_id = %id, "Evicted idle stdio connection");
        }
    }

    // @zen-impl: PLAN-030 Phase 2.2 — spawn background reaper
    /// Spawn a background reaper task that evicts idle stdio connections.
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
    /// Evict the single least-recently-used stdio connection.
    /// Returns `true` if an entry was evicted.
    fn evict_lru_stdio(&self) -> bool {
        let oldest = self
            .connections
            .iter()
            .filter(|e| e.value().transport == TransportType::Stdio)
            .min_by_key(|e| e.value().last_accessed.load(Ordering::Relaxed))
            .map(|e| *e.key());

        if let Some(id) = oldest {
            self.remove(&id);
            info!(server_id = %id, "LRU-evicted stdio connection to make room");
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

/// Execute a tool on an external MCP server.
///
/// 1. Validates the tool exists and the user has access.
/// 2. Connects to the external server (or reuses a pooled connection).
/// 3. Calls the tool with the provided parameters.
/// 4. Records an audit log entry.
/// 5. Returns the result.
pub async fn execute_tool(
    pool: &PgPool,
    client_pool: &ClientPool,
    request: &ExecutionRequest,
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
    let result = execute_with_retry(pool, client_pool, server_id, &call_params).await?;

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
) -> Result<CallToolResult, McpError> {
    // Attempt 1
    client_pool.get_or_connect(pool, server_id).await?;

    match call_tool(client_pool, server_id, params).await {
        Ok(result) => return Ok(result),
        Err(e) => {
            debug!("Tool call failed, retrying after reconnect: {e}");
            client_pool.remove(&server_id);
        }
    }

    // Attempt 2 (reconnect)
    client_pool.get_or_connect(pool, server_id).await?;
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
        assert_eq!(pool.max_stdio_processes, DEFAULT_MAX_STDIO_PROCESSES);
    }

    // @zen-test: PLAN-025 Phase 5.4 — ClientPool with manifest
    #[test]
    fn client_pool_with_manifest_stores_path() {
        let path = PathBuf::from("/tmp/test-manifest");
        let pool = ClientPool::with_manifest(path.clone());
        assert_eq!(pool.manifest_path, Some(path));
    }

    // @zen-test: PLAN-025 Phase 2.3 — max stdio processes configuration
    #[test]
    fn client_pool_set_max_stdio_processes() {
        let mut pool = ClientPool::new();
        pool.set_max_stdio_processes(10);
        assert_eq!(pool.max_stdio_processes, 10);
    }

    // @zen-test: PLAN-025 Phase 2.3 — stdio count tracking
    #[test]
    fn client_pool_stdio_count_tracks_transport_type() {
        let pool = ClientPool::new();
        assert_eq!(pool.stdio_count(), 0);
    }

    // @zen-test: PLAN-025 Phase 2 — Default implementation
    #[test]
    fn client_pool_default_matches_new() {
        let pool = ClientPool::default();
        assert_eq!(pool.connections.len(), 0);
        assert!(pool.manifest_path.is_none());
        assert_eq!(pool.max_stdio_processes, DEFAULT_MAX_STDIO_PROCESSES);
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

    // @zen-test: PLAN-030 Phase 3.1 — evict_lru_stdio returns false on empty pool
    #[test]
    fn evict_lru_stdio_returns_false_on_empty_pool() {
        let pool = ClientPool::new();
        assert!(!pool.evict_lru_stdio());
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
