// @awa-component: CFG-UserSettingsUI
// @awa-impl: CFG-4_AC-1
// @awa-impl: CFG-4_AC-2
// @awa-impl: CFG-4_AC-3
// @awa-impl: CFG-4_AC-4

"use client";

import { useEffect, useState, useCallback } from "react";
import { useAuth, useAuthFetch } from "@/lib/auth-context";

interface ConfigValidator {
  type: string;
  value?: string | number;
  message?: string;
}

interface ResolvedConfigItem {
  key: string;
  category: string;
  type: "number" | "string";
  displayType: "number" | "text" | "longText" | "selector" | "secret";
  possibleValues?: string[];
  validators?: ConfigValidator[];
  defaultValue: string;
  label?: string;
  description?: string;
  value: string | number;
  isOverridden: boolean;
}

export default function SettingsPage() {
  const { isLoading: authLoading, isAuthenticated } = useAuth();
  const authFetch = useAuthFetch();
  const [configs, setConfigs] = useState<ResolvedConfigItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState<string | null>(null);
  const [resetting, setResetting] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [secretInputs, setSecretInputs] = useState<Record<string, string>>({});
  const [secretVisible, setSecretVisible] = useState<Record<string, boolean>>({});

  const loadConfigs = useCallback(async () => {
    try {
      const res = await authFetch("/config/user");
      if (res.ok) {
        const data = await res.json();
        setConfigs(data.items || []);
      } else {
        setError("Failed to load configuration");
      }
    } catch (err) {
      setError("Failed to load configuration");
      console.error(err);
    } finally {
      setLoading(false);
    }
  }, [authFetch]);

  useEffect(() => {
    if (authLoading) return;
    if (!isAuthenticated) return;
    loadConfigs();
  }, [authLoading, isAuthenticated, loadConfigs]);

  const handleUpdate = async (key: string, value: string | number) => {
    setSaving(key);
    setError(null);
    setSuccess(null);

    try {
      const res = await authFetch(`/config/user/${encodeURIComponent(key)}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ value }),
      });

      if (res.ok) {
        const updated = await res.json();
        setConfigs((prev) => prev.map((c) => (c.key === key ? updated : c)));
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

  const handleReset = async (key: string) => {
    setResetting(key);
    setError(null);
    setSuccess(null);

    try {
      const res = await authFetch(`/config/user/${encodeURIComponent(key)}`, {
        method: "DELETE",
      });

      if (res.ok || res.status === 204) {
        await loadConfigs();
        setSuccess(`Reset ${getDisplayLabel(configs.find((c) => c.key === key))} to default`);
        setTimeout(() => setSuccess(null), 3000);
      } else {
        const errorData = await res.json();
        setError(errorData.message || "Failed to reset configuration");
      }
    } catch (err) {
      setError("Failed to reset configuration");
      console.error(err);
    } finally {
      setResetting(null);
    }
  };

  const getDisplayLabel = (config: ResolvedConfigItem | undefined): string => {
    if (!config) return "";
    if (config.label) return config.label;
    const parts = config.key.split(".");
    const label = parts[parts.length - 1];
    return label
      .split(/(?=[A-Z])/)
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(" ");
  };

  const formatCategoryName = (category: string): string => {
    return category.charAt(0).toUpperCase() + category.slice(1) + " Settings";
  };

  const getGroupedConfigs = (): Map<string, ResolvedConfigItem[]> => {
    const grouped = new Map<string, ResolvedConfigItem[]>();
    for (const config of configs) {
      const existing = grouped.get(config.category) || [];
      existing.push(config);
      grouped.set(config.category, existing);
    }
    return grouped;
  };

  const renderInput = (config: ResolvedConfigItem) => {
    const isDisabled = saving === config.key || resetting === config.key;

    switch (config.displayType) {
      case "number":
        return (
          <input
            type="number"
            value={config.value}
            onChange={(e) => {
              const val = e.target.value;
              if (val === "" || val === "-") return;
              const newValue = parseFloat(val);
              if (!isNaN(newValue)) {
                handleUpdate(config.key, newValue);
              }
            }}
            disabled={isDisabled}
            style={{ ...s.input, ...(isDisabled ? s.inputDisabled : {}) }}
          />
        );

      case "text":
        return (
          <input
            type="text"
            defaultValue={config.value}
            onBlur={(e) => {
              if (e.target.value !== String(config.value)) {
                handleUpdate(config.key, e.target.value);
              }
            }}
            disabled={isDisabled}
            style={{ ...s.input, ...(isDisabled ? s.inputDisabled : {}) }}
          />
        );

      case "longText":
        return (
          <textarea
            defaultValue={config.value}
            onBlur={(e) => {
              if (e.target.value !== String(config.value)) {
                handleUpdate(config.key, e.target.value);
              }
            }}
            disabled={isDisabled}
            rows={4}
            style={{ ...s.input, ...s.textarea, ...(isDisabled ? s.inputDisabled : {}) }}
          />
        );

      case "selector":
        return (
          <select value={config.value} onChange={(e) => handleUpdate(config.key, e.target.value)} disabled={isDisabled} style={{ ...s.input, ...(isDisabled ? s.inputDisabled : {}) }}>
            {config.possibleValues?.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        );

      // @awa-impl: PLAN-028-2.1
      case "secret": {
        const currentValue = String(config.value);
        const isConfigured = currentValue.length > 0;
        const inputValue = secretInputs[config.key] ?? "";
        const isVisible = secretVisible[config.key] ?? false;
        return (
          <div>
            <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
              <div style={{ position: "relative", flex: 1 }}>
                <input
                  type={isVisible ? "text" : "password"}
                  value={inputValue}
                  placeholder={isConfigured ? currentValue : "Enter API key..."}
                  onChange={(e) => setSecretInputs((prev) => ({ ...prev, [config.key]: e.target.value }))}
                  onBlur={() => {
                    if (inputValue && inputValue !== currentValue) {
                      handleUpdate(config.key, inputValue);
                      setSecretInputs((prev) => ({ ...prev, [config.key]: "" }));
                    }
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && inputValue) {
                      handleUpdate(config.key, inputValue);
                      setSecretInputs((prev) => ({ ...prev, [config.key]: "" }));
                    }
                  }}
                  disabled={isDisabled}
                  style={{ ...s.input, paddingRight: "2.5rem", ...(isDisabled ? s.inputDisabled : {}) }}
                />
                <button type="button" onClick={() => setSecretVisible((prev) => ({ ...prev, [config.key]: !isVisible }))} style={s.eyeButton} title={isVisible ? "Hide" : "Show"}>
                  {isVisible ? "◉" : "◎"}
                </button>
              </div>
              {isConfigured && (
                <button onClick={() => handleReset(config.key)} disabled={isDisabled} style={s.clearButton} title="Remove key">
                  ✕
                </button>
              )}
            </div>
            {isConfigured && <span style={s.configuredBadge}>✓ Configured</span>}
          </div>
        );
      }

      default:
        return null;
    }
  };

  if (authLoading || loading) {
    return (
      <div style={s.loadingContainer}>
        <span style={{ color: "#666" }}>Loading...</span>
      </div>
    );
  }

  const groupedConfigs = getGroupedConfigs();

  return (
    <div>
      <p style={s.subtitle}>Manage your application configuration</p>

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

      {/* Configuration Groups */}
      <div>
        {Array.from(groupedConfigs.entries()).map(([category, items]) => (
          <div key={category} style={s.card}>
            <h2 style={s.cardTitle}>{formatCategoryName(category)}</h2>
            <div>
              {items.map((config, idx) => (
                <div
                  key={config.key}
                  style={{
                    ...s.configItem,
                    ...(idx < items.length - 1 ? s.configItemBorder : {}),
                  }}
                >
                  <div style={s.configHeader}>
                    <div style={{ flex: 1 }}>
                      <label style={s.configLabel}>
                        {getDisplayLabel(config)}
                        {config.isOverridden && <span style={s.customizedBadge}>(customized)</span>}
                      </label>
                      {config.description && <p style={s.configDescription}>{config.description}</p>}
                    </div>
                    {config.isOverridden && (
                      <button onClick={() => handleReset(config.key)} disabled={resetting === config.key || saving === config.key} style={s.resetButton} title="Reset to default">
                        {resetting === config.key ? "Resetting..." : "↺ Reset"}
                      </button>
                    )}
                  </div>
                  {renderInput(config)}
                  {config.validators && config.validators.length > 0 && <p style={s.validatorHint}>{config.validators.map((v) => v.message || `${v.type}: ${v.value}`).join(", ")}</p>}
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

const s: Record<string, React.CSSProperties> = {
  loadingContainer: {
    display: "flex",
    padding: "3rem 0",
    alignItems: "center",
    justifyContent: "center",
    fontFamily: "system-ui, sans-serif",
  },
  subtitle: {
    fontSize: "0.875rem",
    color: "#666",
    marginTop: "0.25rem",
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
  customizedBadge: {
    marginLeft: "0.5rem",
    fontSize: "0.75rem",
    color: "#2563eb",
    fontWeight: 400,
  },
  configDescription: {
    fontSize: "0.75rem",
    color: "#999",
    marginTop: "0.125rem",
  },
  resetButton: {
    marginLeft: "1rem",
    background: "none",
    border: "none",
    color: "#666",
    cursor: "pointer",
    fontSize: "0.8125rem",
    whiteSpace: "nowrap" as const,
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
  textarea: {
    resize: "vertical" as const,
  },
  validatorHint: {
    fontSize: "0.75rem",
    color: "#999",
    marginTop: "0.25rem",
  },
  eyeButton: {
    position: "absolute" as const,
    right: "0.5rem",
    top: "50%",
    transform: "translateY(-50%)",
    background: "none",
    border: "none",
    cursor: "pointer",
    fontSize: "1rem",
    color: "#666",
    padding: "0.25rem",
  },
  clearButton: {
    background: "none",
    border: "1px solid #d1d5db",
    borderRadius: "6px",
    cursor: "pointer",
    fontSize: "0.875rem",
    color: "#666",
    padding: "0.5rem 0.75rem",
    whiteSpace: "nowrap" as const,
  },
  configuredBadge: {
    display: "inline-block",
    fontSize: "0.75rem",
    color: "#166534",
    marginTop: "0.25rem",
  },
};
