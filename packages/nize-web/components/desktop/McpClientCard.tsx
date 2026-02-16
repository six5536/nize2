// @zen-component: PLAN-011-McpClientCard
// @zen-impl: PLAN-021 — ported from packages/nize-desktop/src/settings/McpClientCard.tsx

"use client";

import { useState } from "react";
import { McpTokenSection } from "./McpTokenSection";

type McpConfigState = "notConfigured" | "needsUpdate" | "configured";

interface McpClientCardProps {
  displayName: string;
  installed: boolean;
  configState: McpConfigState;
  automatable: boolean;
  tokenName: string;
  mcpUrl: string | null;
  onConfigure: () => Promise<void>;
  onRemove: () => Promise<void>;
  createMcpToken?: (name: string) => Promise<{ id: string; token: string }>;
  revokeMcpToken?: (id: string) => Promise<void>;
  listMcpTokens?: () => Promise<{ tokens: Array<{ id: string; name: string; revokedAt?: string | null }> }>;
}

/**
 * Individual MCP client card showing status and actions.
 */
export function McpClientCard({ displayName, installed, configState, automatable, tokenName, mcpUrl, onConfigure, onRemove, createMcpToken, revokeMcpToken, listMcpTokens }: McpClientCardProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showInstructions, setShowInstructions] = useState(false);

  const statusColor = !installed ? "#999" : configState === "configured" ? "#22c55e" : configState === "needsUpdate" ? "#f97316" : "#eab308";
  const statusText = !installed ? "Not Installed" : configState === "configured" ? "Configured ✓" : configState === "needsUpdate" ? "Needs Update" : "Not Configured";

  async function handleConfigure() {
    setLoading(true);
    setError(null);
    try {
      await onConfigure();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleRemove() {
    setLoading(true);
    setError(null);
    try {
      await onRemove();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div
      style={{
        padding: "1rem",
        border: "1px solid #e5e7eb",
        borderRadius: "8px",
        marginBottom: "0.75rem",
        opacity: !installed ? 0.6 : 1,
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <strong>{displayName}</strong>
          <span
            style={{
              marginLeft: "0.75rem",
              fontSize: "0.8rem",
              color: statusColor,
              fontWeight: 600,
            }}
          >
            {statusText}
          </span>
        </div>

        <div style={{ display: "flex", gap: "0.5rem" }}>
          {automatable ? (
            <>
              <button onClick={handleConfigure} disabled={!installed || loading} style={{ fontSize: "0.85rem" }}>
                {loading ? "…" : configState === "configured" ? "Reconfigure" : configState === "needsUpdate" ? "Update" : "Configure"}
              </button>
              {configState !== "notConfigured" && (
                <button onClick={handleRemove} disabled={loading} style={{ fontSize: "0.85rem", color: "#dc2626" }}>
                  Remove
                </button>
              )}
            </>
          ) : (
            <button onClick={() => setShowInstructions(!showInstructions)} style={{ fontSize: "0.85rem" }}>
              {showInstructions ? "Hide Instructions" : "Show Instructions"}
            </button>
          )}
        </div>
      </div>

      {error && <p style={{ color: "red", fontSize: "0.85rem", marginTop: "0.5rem", marginBottom: 0 }}>{error}</p>}

      {showInstructions && !automatable && mcpUrl && (
        <div
          style={{
            marginTop: "0.75rem",
            padding: "0.75rem",
            background: "#f9fafb",
            borderRadius: "4px",
            fontSize: "0.85rem",
          }}
        >
          <p style={{ margin: "0 0 0.5rem" }}>Open ChatGPT → Settings → Connections → Add MCP Server:</p>
          <p style={{ margin: "0 0 0.75rem" }}>
            <strong>URL:</strong> <code style={{ background: "#e5e7eb", padding: "0.15rem 0.3rem", borderRadius: "2px" }}>{mcpUrl}</code>
          </p>
          {createMcpToken && revokeMcpToken && listMcpTokens && <McpTokenSection tokenName={tokenName} createMcpToken={createMcpToken} revokeMcpToken={revokeMcpToken} listMcpTokens={listMcpTokens} />}
        </div>
      )}
    </div>
  );
}
