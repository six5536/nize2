// @zen-component: PLAN-011-McpTokenSection

import { useState, useEffect, useCallback } from "react";

interface McpTokenSectionProps {
  tokenName: string;
  createMcpToken: (name: string) => Promise<{ id: string; token: string }>;
  revokeMcpToken: (id: string) => Promise<void>;
  listMcpTokens: () => Promise<{ tokens: Array<{ id: string; name: string; revokedAt?: string | null }> }>;
}

/**
 * Token management section: generate, display, copy, regenerate, and remove MCP tokens.
 */
export function McpTokenSection({ tokenName, createMcpToken, revokeMcpToken, listMcpTokens }: McpTokenSectionProps) {
  const [tokenValue, setTokenValue] = useState<string | null>(null);
  const [existingTokenId, setExistingTokenId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const checkExisting = useCallback(async () => {
    try {
      const resp = await listMcpTokens();
      const existing = resp.tokens.find((t) => t.name === tokenName && !t.revokedAt);
      setExistingTokenId(existing?.id ?? null);
    } catch {
      // Token check is best-effort
    }
  }, [listMcpTokens, tokenName]);

  useEffect(() => {
    checkExisting();
  }, [checkExisting]);

  async function handleGenerate() {
    setLoading(true);
    setError(null);
    try {
      if (existingTokenId) {
        await revokeMcpToken(existingTokenId);
      }
      const resp = await createMcpToken(tokenName);
      setTokenValue(resp.token);
      setExistingTokenId(resp.id);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleRemove() {
    if (!existingTokenId) return;
    setLoading(true);
    setError(null);
    try {
      await revokeMcpToken(existingTokenId);
      setTokenValue(null);
      setExistingTokenId(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  function handleCopy() {
    if (!tokenValue) return;
    navigator.clipboard.writeText(`Bearer ${tokenValue}`);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div>
      <strong style={{ fontSize: "0.85rem" }}>Authorization Header:</strong>
      {error && <p style={{ color: "red", fontSize: "0.8rem", margin: "0.25rem 0" }}>{error}</p>}

      {tokenValue ? (
        <div style={{ marginTop: "0.5rem" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "0.5rem", flexWrap: "wrap" }}>
            <code
              style={{
                background: "#e5e7eb",
                padding: "0.25rem 0.5rem",
                borderRadius: "4px",
                fontSize: "0.8rem",
                wordBreak: "break-all",
              }}
            >
              Bearer {tokenValue}
            </code>
            <button onClick={handleCopy} style={{ fontSize: "0.8rem" }}>
              {copied ? "Copied!" : "Copy"}
            </button>
          </div>
          <div style={{ marginTop: "0.5rem", display: "flex", gap: "0.5rem" }}>
            <button onClick={handleGenerate} disabled={loading} style={{ fontSize: "0.8rem" }}>
              {loading ? "…" : "Regenerate"}
            </button>
            <button onClick={handleRemove} disabled={loading} style={{ fontSize: "0.8rem", color: "#dc2626" }}>
              Remove Token
            </button>
          </div>
        </div>
      ) : existingTokenId ? (
        <div style={{ marginTop: "0.5rem" }}>
          <p style={{ margin: "0 0 0.5rem", color: "#666", fontSize: "0.85rem" }}>Token active (regenerate to see new value).</p>
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <button onClick={handleGenerate} disabled={loading} style={{ fontSize: "0.8rem" }}>
              {loading ? "…" : "Regenerate"}
            </button>
            <button onClick={handleRemove} disabled={loading} style={{ fontSize: "0.8rem", color: "#dc2626" }}>
              Remove Token
            </button>
          </div>
        </div>
      ) : (
        <div style={{ marginTop: "0.5rem" }}>
          <button onClick={handleGenerate} disabled={loading} style={{ fontSize: "0.85rem" }}>
            {loading ? "Generating…" : "Generate Token"}
          </button>
        </div>
      )}
    </div>
  );
}
