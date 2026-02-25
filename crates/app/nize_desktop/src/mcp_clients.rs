// @awa-component: PLAN-011-McpClients
//! MCP client configuration detection and writing.
//!
//! Detects installed AI clients, checks if Nize is configured,
//! and writes/removes config entries for each client.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::info;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum McpClient {
    ClaudeDesktop,
    ClaudeCode,
    CopilotVscode,
    ChatGptDesktop,
}

impl McpClient {
    pub const ALL: &[McpClient] = &[
        McpClient::ClaudeDesktop,
        McpClient::ClaudeCode,
        McpClient::CopilotVscode,
        McpClient::ChatGptDesktop,
    ];

    /// Human-readable display name.
    pub fn display_name(self) -> &'static str {
        match self {
            McpClient::ClaudeDesktop => "Claude Desktop",
            McpClient::ClaudeCode => "Claude Code",
            McpClient::CopilotVscode => "GitHub Copilot (VS Code)",
            McpClient::ChatGptDesktop => "ChatGPT Desktop",
        }
    }

    /// Token name prefix for this client.
    pub fn token_name(self) -> &'static str {
        match self {
            McpClient::ClaudeDesktop => "nize-claude-desktop",
            McpClient::ClaudeCode => "nize-claude-code",
            McpClient::CopilotVscode => "nize-copilot-vscode",
            McpClient::ChatGptDesktop => "nize-chatgpt",
        }
    }

    /// Whether this client supports automated configuration.
    pub fn is_automatable(self) -> bool {
        !matches!(self, McpClient::ChatGptDesktop)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum McpConfigState {
    NotConfigured,
    NeedsUpdate,
    Configured,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientStatus {
    pub client: McpClient,
    pub display_name: String,
    pub installed: bool,
    pub config_state: McpConfigState,
    pub automatable: bool,
    pub token_name: String,
}

// ---------------------------------------------------------------------------
// Config path resolution (macOS)
// ---------------------------------------------------------------------------

fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Config file path for each client (macOS).
fn config_path(client: McpClient) -> Option<PathBuf> {
    let home = home_dir()?;
    match client {
        McpClient::ClaudeDesktop => Some(
            home.join("Library")
                .join("Application Support")
                .join("Claude")
                .join("claude_desktop_config.json"),
        ),
        McpClient::ClaudeCode => Some(home.join(".claude.json")),
        McpClient::CopilotVscode => Some(
            home.join("Library")
                .join("Application Support")
                .join("Code")
                .join("User")
                .join("mcp.json"),
        ),
        McpClient::ChatGptDesktop => None, // Not automatable
    }
}

/// Directory whose existence indicates the client is installed.
fn install_indicator_path(client: McpClient) -> Option<PathBuf> {
    let home = home_dir()?;
    match client {
        McpClient::ClaudeDesktop => Some(
            home.join("Library")
                .join("Application Support")
                .join("Claude"),
        ),
        McpClient::ClaudeCode => {
            // Claude Code creates ~/.claude/ on install
            let claude_dir = home.join(".claude");
            if claude_dir.exists() {
                return Some(claude_dir);
            }
            // Fallback: check ~/.claude.json
            let claude_json = home.join(".claude.json");
            if claude_json.exists() {
                return Some(claude_json);
            }
            None
        }
        McpClient::CopilotVscode => Some(
            home.join("Library")
                .join("Application Support")
                .join("Code"),
        ),
        McpClient::ChatGptDesktop => {
            let app = PathBuf::from("/Applications/ChatGPT.app");
            if app.exists() { Some(app) } else { None }
        }
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Check if the client appears to be installed.
pub fn is_client_installed(client: McpClient) -> bool {
    install_indicator_path(client).is_some_and(|p| p.exists())
}

/// Check if Nize is configured in this client's config and whether the
/// configuration is valid (matches the expected shape) or stale/outdated.
pub fn get_nize_config_state(client: McpClient) -> McpConfigState {
    let Some(path) = config_path(client) else {
        return McpConfigState::NotConfigured;
    };
    if !path.exists() {
        return McpConfigState::NotConfigured;
    }
    let Ok(content) = fs::read_to_string(&path) else {
        return McpConfigState::NotConfigured;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return McpConfigState::NotConfigured;
    };

    // Look up the "nize" entry in the appropriate top-level object.
    let nize_entry = match client {
        McpClient::ClaudeDesktop | McpClient::ClaudeCode => {
            json.get("mcpServers").and_then(|s| s.get("nize"))
        }
        McpClient::CopilotVscode => json.get("servers").and_then(|s| s.get("nize")),
        McpClient::ChatGptDesktop => return McpConfigState::NotConfigured,
    };

    let Some(entry) = nize_entry else {
        return McpConfigState::NotConfigured;
    };

    // Entry exists — validate the shape matches what configure_* would write.
    let valid = match client {
        McpClient::ClaudeDesktop => validate_claude_desktop_entry(entry),
        McpClient::ClaudeCode | McpClient::CopilotVscode => validate_http_entry(entry),
        McpClient::ChatGptDesktop => false,
    };

    if valid {
        McpConfigState::Configured
    } else {
        McpConfigState::NeedsUpdate
    }
}

/// Validate Claude Desktop stdio-bridge entry:
/// - `command` must be `"bun"` (resolved via env.PATH)
/// - `args[0]` must be the bundled mcp-remote.mjs path
/// - `args[1]` must be an http://127.0.0.1:*/mcp URL
/// - `env.AUTH_TOKEN` must be present
/// - `env.PATH` must be present
fn validate_claude_desktop_entry(entry: &serde_json::Value) -> bool {
    let Some(command) = entry.get("command").and_then(|v| v.as_str()) else {
        return false;
    };
    // Must be bare "bun" command, resolved via PATH env
    if command != "bun" {
        return false;
    }

    let Some(args) = entry.get("args").and_then(|v| v.as_array()) else {
        return false;
    };
    if args.len() < 2 {
        return false;
    }

    // First arg must be the bundled mcp-remote.mjs
    let Some(arg0) = args[0].as_str() else {
        return false;
    };
    if !arg0.ends_with("mcp-remote.mjs") {
        return false;
    }

    // Second arg must be an MCP URL pointing at localhost
    let Some(arg1) = args[1].as_str() else {
        return false;
    };
    if !is_valid_mcp_url(arg1) {
        return false;
    }

    let Some(env) = entry.get("env").and_then(|e| e.as_object()) else {
        return false;
    };

    // Must have env.AUTH_TOKEN
    let has_auth = env
        .get("AUTH_TOKEN")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t.starts_with("Bearer "));

    // Must have env.PATH
    let has_path = env
        .get("PATH")
        .and_then(|v| v.as_str())
        .is_some_and(|p| !p.is_empty());

    has_auth && has_path
}

/// Validate HTTP streamable entry (Claude Code / Copilot VS Code):
/// - `type` must be `"http"`
/// - `url` must be an http://127.0.0.1:*/mcp URL
/// - `headers.Authorization` must be a Bearer token
fn validate_http_entry(entry: &serde_json::Value) -> bool {
    let Some(entry_type) = entry.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    if entry_type != "http" {
        return false;
    }

    let Some(url) = entry.get("url").and_then(|v| v.as_str()) else {
        return false;
    };
    if !is_valid_mcp_url(url) {
        return false;
    }

    entry
        .get("headers")
        .and_then(|h| h.get("Authorization"))
        .and_then(|v| v.as_str())
        .is_some_and(|t| t.starts_with("Bearer "))
}

/// Check that a URL matches the expected `http://127.0.0.1:{port}/mcp` pattern.
fn is_valid_mcp_url(url: &str) -> bool {
    url.starts_with("http://127.0.0.1:") && url.ends_with("/mcp")
}

/// Build status for all clients.
pub fn get_all_statuses() -> Vec<McpClientStatus> {
    McpClient::ALL
        .iter()
        .map(|&client| McpClientStatus {
            client,
            display_name: client.display_name().to_string(),
            installed: is_client_installed(client),
            config_state: get_nize_config_state(client),
            automatable: client.is_automatable(),
            token_name: client.token_name().to_string(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Path resolution helpers
// ---------------------------------------------------------------------------

// @awa-impl: PLAN-016-2.5
/// Resolves the sidecar bun binary path.
pub fn sidecar_bun_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe_dir = exe.parent().ok_or("no parent dir")?;
    let bun = exe_dir.join("bun");
    if bun.exists() {
        Ok(bun)
    } else {
        Err("sidecar bun binary not found".into())
    }
}

// @awa-impl: PLAN-011-2.4
/// Resolves the bundled mcp-remote.mjs path.
pub fn bundled_mcp_remote_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe_dir = exe.parent().ok_or("no parent dir")?;

    // Production macOS .app: Contents/MacOS/exe → Contents/Resources/mcp-remote/
    let resource = exe_dir.parent().map(|p| {
        p.join("Resources")
            .join("mcp-remote")
            .join("mcp-remote.mjs")
    });
    if let Some(ref p) = resource {
        if p.exists() {
            return Ok(p.clone());
        }
    }

    // Dev fallback: look in the nize_desktop resources directory.
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("mcp-remote")
        .join("mcp-remote.mjs");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    Err("bundled mcp-remote.mjs not found".into())
}

// ---------------------------------------------------------------------------
// Config writing
// ---------------------------------------------------------------------------

/// Read a JSON config file, returning an empty object if missing or invalid.
fn read_config(path: &PathBuf) -> serde_json::Value {
    if !path.exists() {
        return serde_json::json!({});
    }
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Write JSON config atomically: write to temp file, then rename.
fn write_config_atomic(path: &PathBuf, value: &serde_json::Value) -> Result<(), String> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create config dir: {e}"))?;
    }

    let content =
        serde_json::to_string_pretty(value).map_err(|e| format!("serialize config: {e}"))?;

    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &content).map_err(|e| format!("write temp config: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename config: {e}"))?;

    Ok(())
}

// @awa-impl: PLAN-016-2.5
/// Configure Claude Desktop: writes mcp-remote stdio bridge config.
pub fn configure_claude_desktop(mcp_port: u16, token: &str) -> Result<(), String> {
    let bun_path = sidecar_bun_path()?;
    let mcp_remote_path = bundled_mcp_remote_path()?;
    let path = config_path(McpClient::ClaudeDesktop).ok_or("no config path")?;

    // Build PATH: bun binary dir + standard system paths.
    let bun_dir = bun_path
        .parent()
        .ok_or("bun binary has no parent dir")?
        .to_string_lossy();
    let env_path = format!("{bun_dir}:/usr/local/bin:/usr/bin:/bin");

    let mut config = read_config(&path);

    let servers = config
        .as_object_mut()
        .ok_or("config is not an object")?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    servers
        .as_object_mut()
        .ok_or("mcpServers is not an object")?
        .insert(
            "nize".to_string(),
            serde_json::json!({
                "command": "bun",
                "args": [
                    mcp_remote_path.to_string_lossy(),
                    format!("http://127.0.0.1:{mcp_port}/mcp"),
                    "--allow-http",
                    "--header",
                    "Authorization:${AUTH_TOKEN}"
                ],
                "env": {
                    "AUTH_TOKEN": format!("Bearer {token}"),
                    "PATH": env_path
                }
            }),
        );

    write_config_atomic(&path, &config)?;
    info!(client = "Claude Desktop", "MCP client configured");
    Ok(())
}

// @awa-impl: PLAN-011-2.3
/// Configure Claude Code: writes HTTP streamable config.
pub fn configure_claude_code(mcp_port: u16, token: &str) -> Result<(), String> {
    let path = config_path(McpClient::ClaudeCode).ok_or("no config path")?;

    let mut config = read_config(&path);

    let servers = config
        .as_object_mut()
        .ok_or("config is not an object")?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    servers
        .as_object_mut()
        .ok_or("mcpServers is not an object")?
        .insert(
            "nize".to_string(),
            serde_json::json!({
                "type": "http",
                "url": format!("http://127.0.0.1:{mcp_port}/mcp"),
                "headers": {
                    "Authorization": format!("Bearer {token}")
                }
            }),
        );

    write_config_atomic(&path, &config)?;
    info!(client = "Claude Code", "MCP client configured");
    Ok(())
}

// @awa-impl: PLAN-011-2.3
/// Configure GitHub Copilot / VS Code: writes HTTP streamable config.
pub fn configure_copilot_vscode(mcp_port: u16, token: &str) -> Result<(), String> {
    let path = config_path(McpClient::CopilotVscode).ok_or("no config path")?;

    let mut config = read_config(&path);

    // VS Code uses "servers" instead of "mcpServers"
    let servers = config
        .as_object_mut()
        .ok_or("config is not an object")?
        .entry("servers")
        .or_insert_with(|| serde_json::json!({}));

    servers
        .as_object_mut()
        .ok_or("servers is not an object")?
        .insert(
            "nize".to_string(),
            serde_json::json!({
                "type": "http",
                "url": format!("http://127.0.0.1:{mcp_port}/mcp"),
                "headers": {
                    "Authorization": format!("Bearer {token}")
                }
            }),
        );

    write_config_atomic(&path, &config)?;
    info!(client = "GitHub Copilot (VS Code)", "MCP client configured");
    Ok(())
}

/// Configure a client by enum variant.
pub fn configure_client(client: McpClient, mcp_port: u16, token: &str) -> Result<(), String> {
    match client {
        McpClient::ClaudeDesktop => configure_claude_desktop(mcp_port, token),
        McpClient::ClaudeCode => configure_claude_code(mcp_port, token),
        McpClient::CopilotVscode => configure_copilot_vscode(mcp_port, token),
        McpClient::ChatGptDesktop => {
            Err("ChatGPT Desktop cannot be configured automatically".into())
        }
    }
}

// ---------------------------------------------------------------------------
// Config removal
// ---------------------------------------------------------------------------

/// Remove the Nize entry from a client's config file.
pub fn remove_nize_from_client(client: McpClient) -> Result<(), String> {
    let path = config_path(client).ok_or("no config path")?;
    if !path.exists() {
        return Ok(()); // Nothing to remove
    }

    let mut config = read_config(&path);

    let key = match client {
        McpClient::ClaudeDesktop | McpClient::ClaudeCode => "mcpServers",
        McpClient::CopilotVscode => "servers",
        McpClient::ChatGptDesktop => return Err("ChatGPT Desktop has no config file".into()),
    };

    if let Some(servers) = config.get_mut(key).and_then(|s| s.as_object_mut()) {
        if servers.remove("nize").is_some() {
            write_config_atomic(&path, &config)?;
            info!(
                client = client.display_name(),
                "Nize entry removed from MCP client config"
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

// @awa-impl: PLAN-011-2.5
#[tauri::command]
pub async fn get_mcp_client_statuses() -> Result<Vec<McpClientStatus>, String> {
    Ok(get_all_statuses())
}

// @awa-impl: PLAN-011-2.5
#[tauri::command]
pub async fn configure_mcp_client(
    client: McpClient,
    mcp_port: u16,
    token: String,
) -> Result<String, String> {
    configure_client(client, mcp_port, &token)?;
    Ok(format!("{} configured successfully", client.display_name()))
}

// @awa-impl: PLAN-011-2.5
#[tauri::command]
pub async fn remove_mcp_client(client: McpClient) -> Result<(), String> {
    remove_nize_from_client(client)
}
