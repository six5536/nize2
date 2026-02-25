// @awa-component: PLAN-023-EmbeddingsConfigUI

/**
 * Admin embedding configuration page at /settings/admin/embeddings
 *
 * Allows admins to configure embedding provider/model settings,
 * view registered models, and re-index all server tools.
 */

"use client";

import { useEffect, useState, useCallback } from "react";
import { useAuth, useAuthFetch } from "@/lib/auth-context";

// =============================================================================
// Types
// =============================================================================

interface ConfigValidator {
  type: string;
  value?: string | number;
  message?: string;
}

interface AdminConfigItem {
  key: string;
  category: string;
  type: "number" | "string";
  displayType: "number" | "text" | "longText" | "selector";
  possibleValues?: string[];
  validators?: ConfigValidator[];
  defaultValue: string;
  label?: string;
  description?: string;
  values: { scope: string; value: string; userId?: string }[];
}

interface EmbeddingModel {
  provider: string;
  name: string;
  dimensions: number;
  tableName: string;
  toolTableName: string;
  isActive: boolean;
}

interface ReindexResult {
  indexed: number;
  serverCount: number;
  errors: { serverId: string; serverName: string; error: string }[];
}

// =============================================================================
// Component
// =============================================================================

export default function EmbeddingsConfigPage() {
  const { isLoading: authLoading, isAuthenticated } = useAuth();
  const authFetch = useAuthFetch();

  const [configs, setConfigs] = useState<AdminConfigItem[]>([]);
  const [models, setModels] = useState<EmbeddingModel[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [reindexing, setReindexing] = useState(false);
  const [reindexResult, setReindexResult] = useState<ReindexResult | null>(null);

  const loadData = useCallback(async () => {
    try {
      const [configRes, modelsRes] = await Promise.all([authFetch("/admin/config?category=embedding"), authFetch("/admin/embeddings/models")]);

      if (configRes.ok) {
        const data = await configRes.json();
        setConfigs(data.items || []);
      } else {
        setError("Failed to load embedding configuration");
      }

      if (modelsRes.ok) {
        const data = await modelsRes.json();
        setModels(data.models || []);
      } else {
        setError("Failed to load embedding models");
      }
    } catch (err) {
      setError("Failed to load data");
      console.error(err);
    } finally {
      setLoading(false);
    }
  }, [authFetch]);

  useEffect(() => {
    if (authLoading) return;
    if (!isAuthenticated) return;
    loadData();
  }, [authLoading, isAuthenticated, loadData]);

  const getSystemValue = (item: AdminConfigItem): string => {
    const systemVal = item.values?.find((v) => v.scope === "system");
    return systemVal?.value ?? item.defaultValue;
  };

  const handleUpdate = async (key: string, value: string) => {
    setSaving(key);
    setError(null);
    setSuccess(null);

    try {
      const res = await authFetch(`/admin/config/system/${encodeURIComponent(key)}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ value }),
      });

      if (res.ok) {
        await loadData();
        setSuccess(`Updated ${getDisplayLabel(configs.find((c) => c.key === key))}`);
        setTimeout(() => setSuccess(null), 3000);
      } else {
        const errorData = await res.json();
        setError(errorData.message || "Failed to update configuration");
      }
    } catch (err) {
      setError("Failed to update configuration");
      console.error(err);
    } finally {
      setSaving(null);
    }
  };

  const handleReindex = async () => {
    setReindexing(true);
    setReindexResult(null);
    setError(null);

    try {
      const res = await authFetch("/admin/embeddings/reindex", {
        method: "POST",
      });

      if (res.ok) {
        const result = await res.json();
        setReindexResult(result);
        // Reload models to reflect any changes
        await loadData();
      } else {
        const errorData = await res.json();
        setError(errorData.message || "Failed to re-index tools");
      }
    } catch (err) {
      setError("Failed to re-index tools");
      console.error(err);
    } finally {
      setReindexing(false);
    }
  };

  const getDisplayLabel = (config: AdminConfigItem | undefined): string => {
    if (!config) return "";
    if (config.label) return config.label;
    const parts = config.key.split(".");
    const label = parts[parts.length - 1];
    return label
      .split(/(?=[A-Z])/)
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(" ");
  };

  const renderInput = (config: AdminConfigItem) => {
    const isDisabled = saving === config.key;
    const currentValue = getSystemValue(config);

    switch (config.displayType) {
      case "selector":
        return (
          <select value={currentValue} onChange={(e) => handleUpdate(config.key, e.target.value)} disabled={isDisabled} style={{ ...s.input, ...(isDisabled ? s.inputDisabled : {}) }}>
            {config.possibleValues?.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        );

      case "text":
        return (
          <input
            type={config.key.toLowerCase().includes("key") ? "password" : "text"}
            defaultValue={currentValue}
            onBlur={(e) => {
              if (e.target.value !== currentValue) {
                handleUpdate(config.key, e.target.value);
              }
            }}
            disabled={isDisabled}
            style={{ ...s.input, ...(isDisabled ? s.inputDisabled : {}) }}
          />
        );

      default:
        return (
          <input
            type="text"
            defaultValue={currentValue}
            onBlur={(e) => {
              if (e.target.value !== currentValue) {
                handleUpdate(config.key, e.target.value);
              }
            }}
            disabled={isDisabled}
            style={{ ...s.input, ...(isDisabled ? s.inputDisabled : {}) }}
          />
        );
    }
  };

  if (authLoading || loading) {
    return (
      <div style={s.loadingContainer}>
        <span style={{ color: "#666" }}>Loading...</span>
      </div>
    );
  }

  return (
    <div>
      <h1 style={s.title}>Embedding Configuration</h1>
      <p style={s.subtitle}>Manage embedding provider, model, and API settings</p>

      {/* Messages */}
      {error && (
        <div style={s.errorBanner} role="alert">
          <p>{error}</p>
        </div>
      )}
      {success && (
        <div style={s.successBanner} role="alert">
          <p>{success}</p>
        </div>
      )}

      {/* Configuration */}
      <div style={s.card}>
        <h2 style={s.cardTitle}>Provider Settings</h2>
        <div>
          {configs.map((config, idx) => (
            <div
              key={config.key}
              style={{
                ...s.configItem,
                ...(idx < configs.length - 1 ? s.configItemBorder : {}),
              }}
            >
              <div style={s.configHeader}>
                <div style={{ flex: 1 }}>
                  <label style={s.configLabel}>{getDisplayLabel(config)}</label>
                  {config.description && <p style={s.configDescription}>{config.description}</p>}
                </div>
              </div>
              {renderInput(config)}
            </div>
          ))}
          {configs.length === 0 && <p style={{ color: "#999", fontSize: "0.875rem" }}>No embedding configuration found. Run database migrations first.</p>}
        </div>
      </div>

      {/* Registered Models */}
      <div style={s.card}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "1rem" }}>
          <h2 style={{ ...s.cardTitle, margin: 0 }}>Registered Models</h2>
          <button onClick={handleReindex} disabled={reindexing} style={s.reindexButton}>
            {reindexing ? "Re-indexing..." : "Re-index All Tools"}
          </button>
        </div>

        {reindexResult && (
          <div
            style={{
              ...(reindexResult.errors.length > 0 ? s.errorBanner : s.successBanner),
              marginBottom: "1rem",
            }}
          >
            <p>
              Indexed {reindexResult.indexed} tool(s) across {reindexResult.serverCount} server(s).
              {reindexResult.errors.length > 0 && <span> {reindexResult.errors.length} error(s):</span>}
            </p>
            {reindexResult.errors.map((err, i) => (
              <p key={i} style={{ fontSize: "0.75rem", marginTop: "0.25rem" }}>
                {err.serverName}: {err.error}
              </p>
            ))}
          </div>
        )}

        {models.length > 0 ? (
          <table style={s.table}>
            <thead>
              <tr>
                <th style={s.th}>Provider</th>
                <th style={s.th}>Model</th>
                <th style={s.th}>Dimensions</th>
                <th style={s.th}>Tool Table</th>
                <th style={s.th}>Status</th>
              </tr>
            </thead>
            <tbody>
              {models.map((model) => (
                <tr key={`${model.provider}-${model.name}`}>
                  <td style={s.td}>{model.provider}</td>
                  <td style={s.td}>
                    <span style={{ fontFamily: "monospace" }}>{model.name}</span>
                  </td>
                  <td style={s.td}>{model.dimensions}</td>
                  <td style={s.td}>
                    <span style={{ fontFamily: "monospace", fontSize: "0.75rem" }}>{model.toolTableName}</span>
                  </td>
                  <td style={s.td}>{model.isActive ? <span style={s.activeBadge}>Active</span> : <span style={s.inactiveBadge}>Inactive</span>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p style={{ color: "#999", fontSize: "0.875rem" }}>No embedding models registered. Run database migrations first.</p>
        )}
      </div>
    </div>
  );
}

// =============================================================================
// Styles
// =============================================================================

const s: Record<string, React.CSSProperties> = {
  loadingContainer: {
    display: "flex",
    padding: "3rem 0",
    alignItems: "center",
    justifyContent: "center",
    fontFamily: "system-ui, sans-serif",
  },
  title: {
    fontSize: "1.5rem",
    fontWeight: 700,
    color: "#111",
    margin: 0,
  },
  subtitle: {
    fontSize: "0.875rem",
    color: "#666",
    marginTop: "0.25rem",
    marginBottom: "1.5rem",
  },
  errorBanner: {
    backgroundColor: "#fef2f2",
    border: "1px solid #fecaca",
    color: "#991b1b",
    padding: "0.75rem 1rem",
    borderRadius: "6px",
    marginBottom: "1rem",
    fontSize: "0.875rem",
  },
  successBanner: {
    backgroundColor: "#f0fdf4",
    border: "1px solid #bbf7d0",
    color: "#166534",
    padding: "0.75rem 1rem",
    borderRadius: "6px",
    marginBottom: "1rem",
    fontSize: "0.875rem",
  },
  card: {
    backgroundColor: "#fff",
    borderRadius: "8px",
    boxShadow: "0 1px 4px rgba(0,0,0,0.08)",
    padding: "1.5rem",
    marginBottom: "1.5rem",
  },
  cardTitle: {
    fontSize: "1.125rem",
    fontWeight: 600,
    color: "#111",
    margin: "0 0 1rem 0",
  },
  configItem: {
    paddingBottom: "1rem",
    marginBottom: "1rem",
  },
  configItemBorder: {
    borderBottom: "1px solid #f0f0f0",
  },
  configHeader: {
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "space-between",
    marginBottom: "0.5rem",
  },
  configLabel: {
    display: "block",
    fontSize: "0.875rem",
    fontWeight: 500,
    color: "#333",
  },
  configDescription: {
    fontSize: "0.75rem",
    color: "#999",
    marginTop: "0.125rem",
  },
  input: {
    width: "100%",
    padding: "0.5rem 0.75rem",
    border: "1px solid #d1d5db",
    borderRadius: "6px",
    fontSize: "0.875rem",
    outline: "none",
    boxSizing: "border-box" as const,
  },
  inputDisabled: {
    backgroundColor: "#f5f5f5",
    opacity: 0.7,
  },
  reindexButton: {
    padding: "0.5rem 1rem",
    backgroundColor: "#3b82f6",
    color: "#fff",
    border: "none",
    borderRadius: "6px",
    fontSize: "0.875rem",
    fontWeight: 500,
    cursor: "pointer",
    whiteSpace: "nowrap" as const,
  },
  table: {
    width: "100%",
    borderCollapse: "collapse" as const,
    fontSize: "0.875rem",
  },
  th: {
    textAlign: "left" as const,
    padding: "0.5rem 0.75rem",
    borderBottom: "2px solid #e5e7eb",
    fontWeight: 600,
    color: "#374151",
    fontSize: "0.75rem",
    textTransform: "uppercase" as const,
    letterSpacing: "0.05em",
  },
  td: {
    padding: "0.5rem 0.75rem",
    borderBottom: "1px solid #f3f4f6",
    color: "#111",
  },
  activeBadge: {
    display: "inline-block",
    padding: "0.125rem 0.5rem",
    backgroundColor: "#dcfce7",
    color: "#166534",
    borderRadius: "9999px",
    fontSize: "0.75rem",
    fontWeight: 500,
  },
  inactiveBadge: {
    display: "inline-block",
    padding: "0.125rem 0.5rem",
    backgroundColor: "#f3f4f6",
    color: "#6b7280",
    borderRadius: "9999px",
    fontSize: "0.75rem",
    fontWeight: 500,
  },
};
