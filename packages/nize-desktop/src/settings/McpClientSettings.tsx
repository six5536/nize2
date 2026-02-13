// @zen-component: PLAN-011-McpClientSettings
// @zen-impl: PLAN-011-3.1

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "../auth";
import { McpClientCard } from "./McpClientCard";
import { McpTokenSection } from "./McpTokenSection";

// Types matching Rust McpClientStatus
interface McpClientStatus {
  client: string;
  displayName: string;
  installed: boolean;
  configState: "notConfigured" | "needsUpdate" | "configured";
  automatable: boolean;
  tokenName: string;
}

/**
 * MCP Client configuration panel.
 * Shows detected AI clients and allows one-click configuration.
 */
export function McpClientSettings() {
  const { createMcpToken, listMcpTokens, revokeMcpToken } = useAuth();
  const [statuses, setStatuses] = useState<McpClientStatus[]>([]);
  const [mcpPort, setMcpPort] = useState<number | null>(null);
  const [mcpUrl, setMcpUrl] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<McpClientStatus[]>("get_mcp_client_statuses");
      setStatuses(result);
      setLoadError(null);
    } catch (e) {
      setLoadError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    invoke<number>("get_mcp_port")
      .then((port) => {
        setMcpPort(port);
        setMcpUrl(`http://127.0.0.1:${port}/mcp`);
      })
      .catch(() => {
        // MCP port not available yet
      });
  }, [refresh]);

  async function handleConfigure(status: McpClientStatus) {
    if (mcpPort == null) {
      throw new Error("MCP port not available yet");
    }
    // Create a dedicated token for this client
    const tokenResp = await createMcpToken(status.tokenName);
    await invoke("configure_mcp_client", {
      client: status.client,
      mcpPort: mcpPort,
      token: tokenResp.token,
    });
    await refresh();
  }

  async function handleRemove(status: McpClientStatus) {
    await invoke("remove_mcp_client", { client: status.client });
    await refresh();
  }

  return (
    <section>
      <h2 style={{ fontSize: "1.1rem", marginTop: 0 }}>MCP Clients</h2>
      <p style={{ color: "#666", fontSize: "0.9rem", marginBottom: "1rem" }}>Configure AI assistants to connect to Nize via MCP.</p>

      {loadError && <p style={{ color: "red", fontSize: "0.85rem" }}>Failed to load client statuses: {loadError}</p>}

      {statuses.map((status) => (
        <McpClientCard key={status.client} displayName={status.displayName} installed={status.installed} configState={status.configState} automatable={status.automatable} tokenName={status.tokenName} mcpUrl={mcpUrl} onConfigure={() => handleConfigure(status)} onRemove={() => handleRemove(status)} createMcpToken={createMcpToken} revokeMcpToken={revokeMcpToken} listMcpTokens={listMcpTokens} />
      ))}

      {/* Custom / Other Client */}
      {mcpUrl && (
        <div
          style={{
            padding: "1rem",
            border: "1px solid #e5e7eb",
            borderRadius: "8px",
            marginBottom: "0.75rem",
          }}
        >
          <div style={{ marginBottom: "0.75rem" }}>
            <strong>Custom / Other Client</strong>
          </div>
          <p style={{ color: "#666", fontSize: "0.85rem", margin: "0 0 0.5rem" }}>For clients not listed above, use these details:</p>
          <p style={{ margin: "0 0 0.75rem", fontSize: "0.85rem" }}>
            <strong>URL:</strong> <code style={{ background: "#e5e7eb", padding: "0.15rem 0.3rem", borderRadius: "2px" }}>{mcpUrl}</code>
          </p>
          <McpTokenSection tokenName="nize-custom" createMcpToken={createMcpToken} revokeMcpToken={revokeMcpToken} listMcpTokens={listMcpTokens} />
        </div>
      )}
    </section>
  );
}
