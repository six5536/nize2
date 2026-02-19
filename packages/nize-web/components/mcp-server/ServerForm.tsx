// @zen-component: PLAN-032-ServerForm

/**
 * Unified MCP server form for both create and edit modes.
 *
 * Implements idiomatic OAuth workflow:
 * - Test Connection: auto-initiates OAuth if authRequired → tests → reports
 * - Save/Create: saves config → revoke if OAuth changed → auth if needed → test
 * - Re-authorize: handled by OAuthStatusBanner (revoke → auth → test)
 *
 * Props control which sections are visible:
 * - showTransport: show transport selector (admin only)
 * - showVisibility: show visibility selector (admin only)
 */

"use client";

import { useState } from "react";
import type { ServerConfig, ServerFormValues, TestConnectionResult } from "./types";
import { useServerForm } from "./useServerForm";
import { useOAuthFlow } from "./useOAuthFlow";
import { HttpConfigFields } from "./HttpConfigFields";
import { StdioConfigFields } from "./StdioConfigFields";
import { OAuthConfigFields } from "./OAuthConfigFields";
import { OAuthStatusBanner } from "./OAuthStatusBanner";

interface ServerFormProps {
  mode: "create" | "edit";
  initialValues?: ServerFormValues;
  /** Show transport selector (admin). */
  showTransport?: boolean;
  /** Show visibility selector (admin). */
  showVisibility?: boolean;
  /** Authenticated fetch function. */
  authFetch: (path: string, options?: RequestInit) => Promise<Response>;

  // --- Callbacks ---
  /** Test connection. Returns result from backend. */
  onTestConnection: (config: ServerConfig, serverId?: string) => Promise<TestConnectionResult>;
  /** Create a new server. Returns the new server's ID. */
  onCreateServer?: (payload: { name: string; description: string; domain: string; visibility: "hidden" | "visible"; config: ServerConfig; oauthConfig?: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] }; clientSecret?: string }) => Promise<string>;
  /** Update an existing server. */
  onUpdateServer?: (
    serverId: string,
    updates: {
      name?: string;
      description?: string;
      domain?: string;
      visibility?: "hidden" | "visible";
      config?: ServerConfig;
      oauthConfig?: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] };
      clientSecret?: string;
    },
  ) => Promise<void>;
  /** Delete a server (used to clean up on failed create+auth). */
  onDeleteServer?: (serverId: string) => Promise<void>;
  /** Called when the form should close. */
  onCancel: () => void;
  /** Called after successful save/create to refresh parent state. */
  onSuccess?: () => void;
}

// @zen-impl: PLAN-032 Step 7
// @zen-impl: PLAN-032 Step 8
export function ServerForm({ mode, initialValues, showTransport = false, showVisibility = false, authFetch, onTestConnection, onCreateServer, onUpdateServer, onDeleteServer, onCancel, onSuccess }: ServerFormProps) {
  const form = useServerForm(initialValues, { mode });
  const { inProgress: oauthInProgress, startOAuthFlow } = useOAuthFlow();

  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<TestConnectionResult | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Track server ID created during OAuth create flow
  const [createdServerId, setCreatedServerId] = useState<string | null>(null);

  // ----- Initiate OAuth for a given server ID -----
  const initiateAndRunOAuth = async (serverId: string): Promise<{ success: boolean; error?: string }> => {
    const oauthRes = await authFetch(`/mcp/servers/${serverId}/oauth/initiate`, { method: "POST" });
    if (!oauthRes.ok) {
      const data = await oauthRes.json().catch(() => ({}));
      return { success: false, error: data.message || "Failed to initiate OAuth" };
    }
    const { authUrl } = await oauthRes.json();
    return startOAuthFlow(authUrl, serverId, authFetch);
  };

  // @zen-impl: PLAN-032 Step 8 — Test Connection: auto-auth if needed → test
  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    setError(null);
    try {
      const config = form.buildConfig();
      let serverId = initialValues?.id || createdServerId || undefined;

      // For create mode with OAuth: must create server first
      if (mode === "create" && form.authType === "oauth" && !createdServerId) {
        if (!onCreateServer) throw new Error("onCreateServer is required for create mode");
        const newId = await onCreateServer({
          name: form.name,
          description: form.description,
          domain: form.domain,
          visibility: form.visibility,
          config,
          oauthConfig: form.buildOAuthConfig(),
          clientSecret: form.clientSecret || undefined,
        });
        setCreatedServerId(newId);
        serverId = newId;
      }

      // First attempt: test connection
      let result = await onTestConnection(config, serverId);

      // @zen-impl: PLAN-032 Step 8 — auto-initiate OAuth if authRequired
      if (result.authRequired && serverId) {
        const oauthResult = await initiateAndRunOAuth(serverId);
        if (oauthResult.success) {
          // Re-test after successful auth
          result = await onTestConnection(config, serverId);
        } else {
          // OAuth failed — clean up server if we just created it
          if (mode === "create" && createdServerId) {
            await onDeleteServer?.(createdServerId).catch(() => {});
            setCreatedServerId(null);
          }
          setTestResult({ success: false, error: oauthResult.error || "OAuth authorization failed" });
          return;
        }
      }

      setTestResult(result);
    } catch (err) {
      setTestResult({ success: false, error: err instanceof Error ? err.message : "Test failed" });
    } finally {
      setTesting(false);
    }
  };

  // @zen-impl: PLAN-032 Step 8 — Save/Create: save → revoke if changed → auth → test
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    // If server was already created during OAuth test flow, just close
    if (mode === "create" && createdServerId) {
      onSuccess?.();
      onCancel();
      return;
    }

    setError(null);
    setSubmitting(true);
    try {
      const config = form.buildConfig();
      const oauthConfig = form.buildOAuthConfig();

      if (mode === "create") {
        if (!onCreateServer) throw new Error("onCreateServer is required for create mode");
        const newId = await onCreateServer({
          name: form.name,
          description: form.description,
          domain: form.domain,
          visibility: form.visibility,
          config,
          oauthConfig,
          clientSecret: form.clientSecret || undefined,
        });

        // For OAuth: auto-auth after create
        if (form.authType === "oauth") {
          setCreatedServerId(newId);
          const oauthResult = await initiateAndRunOAuth(newId);
          if (!oauthResult.success) {
            // Server created but auth failed — keep server, show status
            setError(`Server created. ${oauthResult.error || "OAuth authorization required."}`);
            setSubmitting(false);
            return;
          }
        }

        onSuccess?.();
        onCancel();
      } else {
        // Edit mode
        if (!onUpdateServer || !initialValues?.id) throw new Error("onUpdateServer and initialValues required for edit mode");

        // 1. If OAuth settings changed, revoke old token
        if (form.hasOAuthConfigChanged) {
          await authFetch(`/mcp/servers/${initialValues.id}/oauth/revoke`, { method: "POST" }).catch(() => {});
        }

        // 2. Save config
        await onUpdateServer(initialValues.id, {
          name: form.name,
          description: form.description,
          domain: form.domain,
          visibility: form.visibility,
          config,
          oauthConfig,
          clientSecret: form.clientSecret || undefined,
        });

        // 3. If OAuth server, ensure valid connection
        if (form.authType === "oauth") {
          const testResult = await onTestConnection(config, initialValues.id);
          if (testResult.authRequired) {
            const oauthResult = await initiateAndRunOAuth(initialValues.id);
            if (!oauthResult.success) {
              setError(`Saved. ${oauthResult.error || "OAuth authorization required."}`);
              setSubmitting(false);
              return;
            }
          }
        }

        onSuccess?.();
        onCancel();
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : mode === "create" ? "Failed to create server" : "Failed to update server");
    } finally {
      setSubmitting(false);
    }
  };

  const isBusy = testing || submitting || oauthInProgress;
  const title = mode === "create" ? "Create Built-in Server" : `Edit Server: ${initialValues?.name || ""}`;
  const submitLabel = mode === "create" ? (createdServerId ? "Done" : submitting ? "Creating..." : "Create Server") : submitting ? "Saving..." : "Save Changes";
  const testLabel = testing ? (form.authType === "oauth" ? "Connecting..." : "Testing...") : form.authType === "oauth" && mode === "create" && !createdServerId ? "Connect with OAuth" : "Test Connection";

  return (
    <form onSubmit={handleSubmit} className="border rounded-lg p-6 bg-white shadow-sm space-y-4">
      <h3 className="text-lg font-medium text-gray-900">{title}</h3>

      {/* Common fields */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700">Name</label>
          <input type="text" value={form.name} onChange={(e) => form.setName(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="File System Server" required />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700">Domain</label>
          <input type="text" value={form.domain} onChange={(e) => form.setDomain(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="files" required />
        </div>
      </div>

      <div>
        <label className="block text-sm font-medium text-gray-700">Description</label>
        <textarea value={form.description} onChange={(e) => form.setDescription(e.target.value.slice(0, 500))} rows={2} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Provides file system access for reading and writing files" />
        <p className="mt-1 text-sm text-gray-500">{form.description.length}/500</p>
      </div>

      {/* Visibility & Transport selectors (admin only) */}
      {(showVisibility || showTransport) && (
        <div className="grid grid-cols-2 gap-4">
          {showVisibility && (
            <div>
              <label className="block text-sm font-medium text-gray-700">Visibility</label>
              <select value={form.visibility} onChange={(e) => form.setVisibility(e.target.value as "hidden" | "visible")} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
                <option value="visible">Visible (Users can toggle)</option>
                <option value="hidden">Hidden (System only)</option>
              </select>
            </div>
          )}
          {showTransport && (
            <div>
              <label className="block text-sm font-medium text-gray-700">Transport</label>
              <select value={form.transport} onChange={(e) => form.setTransport(e.target.value as "stdio" | "http")} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
                <option value="http">HTTP (Remote)</option>
                <option value="stdio">Stdio (Local)</option>
              </select>
            </div>
          )}
        </div>
      )}

      {/* Transport-specific fields */}
      {form.transport === "http" ? (
        <>
          <HttpConfigFields url={form.url} authType={form.authType} apiKey={form.apiKey} onUrlChange={form.setUrl} onAuthTypeChange={form.setAuthType} onApiKeyChange={form.setApiKey} apiKeyPlaceholder={mode === "edit" ? "Enter new API key (leave blank to keep existing)" : "Enter API key"} />
          {form.authType === "oauth" && (
            <>
              {/* OAuth status banner — only for existing servers */}
              {mode === "edit" && initialValues?.id && <OAuthStatusBanner serverId={initialValues.id} authFetch={authFetch} onError={setError} />}
              {/* OAuth config changed warning */}
              {form.hasOAuthConfigChanged && mode === "edit" && (
                <div className="p-3 rounded-md bg-orange-50 border border-orange-200">
                  <p className="text-sm font-medium text-orange-800">&#9888; OAuth settings changed — saving will disconnect the current session and require re-authorization.</p>
                </div>
              )}
              <OAuthConfigFields clientId={form.clientId} clientSecret={form.clientSecret} oauthScopes={form.oauthScopes} authorizationUrl={form.authorizationUrl} tokenUrl={form.tokenUrl} onClientIdChange={form.setClientId} onClientSecretChange={form.setClientSecret} onOauthScopesChange={form.setOauthScopes} onAuthorizationUrlChange={form.setAuthorizationUrl} onTokenUrlChange={form.setTokenUrl} clientSecretPlaceholder={mode === "edit" ? "Leave blank to keep existing" : "Google OAuth Client Secret"} clientSecretRequired={mode === "create"} />
            </>
          )}
        </>
      ) : (
        <StdioConfigFields command={form.command} args={form.args} envPairs={form.envPairs} onCommandChange={form.setCommand} onArgsChange={form.setArgs} onEnvPairsChange={form.setEnvPairs} />
      )}

      {/* Test result */}
      {testResult && <div className={`p-3 rounded-md ${testResult.success ? "bg-green-50 text-green-800" : "bg-red-50 text-red-800"}`}>{testResult.success ? <p>&#10003; Connected successfully!{testResult.toolCount != null && ` Found ${testResult.toolCount} tools.`}</p> : <p>&#10007; {testResult.error}</p>}</div>}

      {/* Error */}
      {error && <div className="p-3 rounded-md bg-red-50 text-red-800">{error}</div>}

      {/* Footer buttons */}
      <div className="flex gap-3 justify-end">
        <button
          type="button"
          onClick={async () => {
            // Clean up orphaned server on cancel during create flow
            if (mode === "create" && createdServerId && !testResult?.success) {
              await onDeleteServer?.(createdServerId).catch(() => {});
            }
            onCancel();
          }}
          className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50"
        >
          Cancel
        </button>
        <button type="button" onClick={handleTest} disabled={!form.isValid || isBusy} className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50 disabled:opacity-50">
          {testLabel}
        </button>
        <button type="submit" disabled={!form.isValid || isBusy || (mode === "create" && !createdServerId && form.authType === "oauth" && !testResult?.success)} className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50">
          {submitLabel}
        </button>
      </div>
    </form>
  );
}
