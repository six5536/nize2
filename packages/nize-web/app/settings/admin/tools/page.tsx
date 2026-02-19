// @zen-component: PLAN-020-AdminToolsUI

/**
 * Admin MCP server management page at /settings/admin/tools
 *
 * Allows admins to view, create, edit, toggle, and delete
 * built-in MCP server configurations system-wide.
 */

"use client";

import { useEffect, useState, useCallback } from "react";
import { useAuth, useAuthFetch } from "@/lib/auth-context";
import { openExternal } from "@/lib/tauri";

// =============================================================================
// Types
// =============================================================================

type ServerStatus = "enabled" | "disabled" | "unavailable" | "unauthorized";
type VisibilityTier = "hidden" | "visible" | "user";
type TransportType = "stdio" | "http";
type AuthType = "none" | "api-key" | "oauth";

interface AdminServerView {
  id: string;
  name: string;
  description: string;
  domain: string;
  visibility: VisibilityTier;
  transport: TransportType;
  authType: AuthType;
  status: ServerStatus;
  toolCount: number;
  ownerId: string | null;
  isOwned: boolean;
  userPreferenceCount: number;
  enabled: boolean;
  available: boolean;
  config?: Record<string, unknown>;
  oauthConfig?: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] };
}

// =============================================================================
// Components
// =============================================================================

function AdminServerList({ servers, groupBy, onEdit, onDelete, onToggleEnabled }: { servers: AdminServerView[]; groupBy: "visibility" | "transport"; onEdit: (serverId: string) => void; onDelete: (serverId: string) => void; onToggleEnabled: (serverId: string, enabled: boolean) => void }) {
  const grouped = servers.reduce(
    (acc, server) => {
      const key = groupBy === "visibility" ? server.visibility : server.transport;
      if (!acc[key]) acc[key] = [];
      acc[key].push(server);
      return acc;
    },
    {} as Record<string, AdminServerView[]>,
  );

  const visibilityLabels: Record<VisibilityTier, string> = {
    hidden: "Hidden (System)",
    visible: "Visible (Default)",
    user: "User-Owned",
  };

  const transportLabels: Record<TransportType, string> = {
    stdio: "Stdio (Local)",
    http: "HTTP (Remote)",
  };

  const labels = groupBy === "visibility" ? visibilityLabels : transportLabels;
  const order = groupBy === "visibility" ? ["hidden", "visible", "user"] : ["stdio", "http"];

  return (
    <div className="space-y-8">
      {order.map((key) => {
        const groupServers = grouped[key] || [];
        if (groupServers.length === 0) return null;

        return (
          <div key={key}>
            <h3 className="text-lg font-medium text-gray-900 mb-4">{labels[key as keyof typeof labels]}</h3>
            <div className="bg-white shadow overflow-hidden rounded-md">
              <ul className="divide-y divide-gray-200">
                {groupServers.map((server) => (
                  <li key={server.id} className="px-6 py-4">
                    <div className="flex items-center justify-between">
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <h4 className="text-sm font-medium text-gray-900 truncate">{server.name}</h4>
                          {server.visibility === "user" && <span className="px-2 py-0.5 text-xs bg-purple-100 text-purple-800 rounded">User: {server.ownerId?.slice(0, 8)}...</span>}
                        </div>
                        <div className="flex items-center gap-4 mt-1">
                          <span className="text-sm text-gray-500">Domain: {server.domain}</span>
                          <span className="text-sm text-gray-500">Transport: {server.transport}</span>
                          <span className="text-sm text-gray-500">Auth: {server.authType}</span>
                          <span className="text-sm text-gray-500">{server.toolCount} tools</span>
                          {server.visibility !== "user" && server.userPreferenceCount > 0 && <span className="text-sm text-orange-600">{server.userPreferenceCount} users enabled</span>}
                        </div>
                      </div>

                      <div className="flex items-center gap-3">
                        <span className={`px-2 py-1 text-xs font-medium rounded-full ${server.status === "enabled" ? "bg-green-100 text-green-800" : server.status === "disabled" ? "bg-gray-100 text-gray-800" : "bg-red-100 text-red-800"}`}>{server.status}</span>

                        {server.visibility !== "user" && (
                          <label className="relative inline-flex items-center cursor-pointer">
                            <input type="checkbox" className="sr-only peer" checked={server.status === "enabled"} onChange={(e) => onToggleEnabled(server.id, e.target.checked)} />
                            <div className="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
                          </label>
                        )}

                        {server.visibility === "user" ? (
                          <button className="p-1 text-gray-400 cursor-not-allowed" title="User servers are view-only" disabled>
                            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                            </svg>
                          </button>
                        ) : (
                          <>
                            <button onClick={() => onEdit(server.id)} className="p-1 text-gray-500 hover:text-gray-700" title="Edit server">
                              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                              </svg>
                            </button>
                            <button onClick={() => onDelete(server.id)} className="p-1 text-red-500 hover:text-red-700" title="Delete server">
                              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                              </svg>
                            </button>
                          </>
                        )}
                      </div>
                    </div>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        );
      })}
    </div>
  );
}

function CreateServerForm({
  onSubmit,
  onTest,
  onOAuthConnect,
  onDeleteServer,
  onCancel,
  authFetch,
}: {
  onSubmit: (config: { name: string; domain: string; description: string; visibility: "hidden" | "visible"; config: { transport: "stdio"; command: string; args?: string[]; env?: Record<string, string> } | { transport: "http"; url: string; authType: AuthType; apiKey?: string }; oauthConfig?: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] }; clientSecret?: string }) => Promise<void>;
  onTest: (config: { transport: "stdio"; command: string; args?: string[]; env?: Record<string, string> } | { transport: "http"; url: string; authType: AuthType; apiKey?: string }) => Promise<{ success: boolean; toolCount?: number; error?: string }>;
  onOAuthConnect: (formData: { name: string; domain: string; description: string; visibility: "hidden" | "visible"; config: { transport: "http"; url: string; authType: "oauth" }; oauthConfig: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] }; clientSecret: string }) => Promise<{ serverId: string; authUrl: string }>;
  onDeleteServer: (serverId: string) => Promise<void>;
  onCancel: () => void;
  authFetch: (path: string, options?: RequestInit) => Promise<Response>;
}) {
  const [name, setName] = useState("");
  const [domain, setDomain] = useState("");
  const [description, setDescription] = useState("");
  const [visibility, setVisibility] = useState<"hidden" | "visible">("visible");
  const [transport, setTransport] = useState<TransportType>("http");

  // HTTP config
  const [url, setUrl] = useState("");
  const [authType, setAuthType] = useState<AuthType>("none");
  const [apiKey, setApiKey] = useState("");

  // OAuth config
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [authorizationUrl, setAuthorizationUrl] = useState("https://accounts.google.com/o/oauth2/v2/auth");
  const [tokenUrl, setTokenUrl] = useState("https://oauth2.googleapis.com/token");
  const [oauthScopes, setOauthScopes] = useState("openid email profile");
  const [createdServerId, setCreatedServerId] = useState<string | null>(null);

  // Stdio config
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [envPairs, setEnvPairs] = useState<{ key: string; value: string }[]>([]);

  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; toolCount?: number; error?: string } | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const buildConfig = () => {
    if (transport === "stdio") {
      const env: Record<string, string> = {};
      for (const pair of envPairs) {
        if (pair.key) env[pair.key] = pair.value;
      }
      return {
        transport: "stdio" as const,
        command,
        args: args ? args.split(/\s+/) : undefined,
        env: Object.keys(env).length > 0 ? env : undefined,
      };
    } else {
      return {
        transport: "http" as const,
        url,
        authType,
        apiKey: authType === "api-key" ? apiKey : undefined,
      };
    }
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      if (authType === "oauth") {
        // OAuth flow: create server, then open authorization popup
        const { serverId, authUrl } = await onOAuthConnect({
          name,
          domain,
          description,
          visibility,
          config: { transport: "http" as const, url, authType: "oauth" as const },
          oauthConfig: {
            clientId,
            authorizationUrl,
            tokenUrl,
            scopes: oauthScopes.split(/[\s,]+/).filter(Boolean),
          },
          clientSecret,
        });
        setCreatedServerId(serverId);

        // Open OAuth in system browser (Tauri) or popup (browser)
        const popup = await openExternal(authUrl, "oauth-connect", "width=600,height=700");

        let result: { success: boolean; error?: string };

        if (popup) {
          // Browser: listen for postMessage from OAuth callback
          result = await new Promise<{ success: boolean; error?: string }>((resolve) => {
            const cleanup = () => {
              window.removeEventListener("message", listener);
              clearInterval(pollClosed);
              clearTimeout(timeout);
            };
            const listener = (event: MessageEvent) => {
              if (event.data?.type === "oauth-success" && event.data.serverId === serverId) {
                cleanup();
                resolve({ success: true });
              } else if (event.data?.type === "oauth-error") {
                cleanup();
                resolve({ success: false, error: event.data.error || "OAuth authorization failed" });
              }
            };
            window.addEventListener("message", listener);

            // Timeout after 2 minutes
            const timeout = setTimeout(() => {
              cleanup();
              if (popup && !popup.closed) popup.close();
              resolve({ success: false, error: "OAuth authorization timed out" });
            }, 120000);

            // Poll for popup closed without completing
            const pollClosed = setInterval(() => {
              if (popup && popup.closed) {
                cleanup();
                resolve({ success: false, error: "OAuth window was closed" });
              }
            }, 500);
          });
        } else {
          // Tauri / no popup: poll OAuth status endpoint until connected
          result = await new Promise<{ success: boolean; error?: string }>((resolve) => {
            let stopped = false;
            const timeout = setTimeout(() => {
              stopped = true;
              clearInterval(poll);
              resolve({ success: false, error: "OAuth authorization timed out" });
            }, 120000);
            const poll = setInterval(async () => {
              if (stopped) return;
              try {
                const res = await authFetch(`/mcp/servers/${serverId}/oauth/status`);
                if (res.ok) {
                  const status = await res.json();
                  if (status.connected) {
                    stopped = true;
                    clearTimeout(timeout);
                    clearInterval(poll);
                    resolve({ success: true });
                  }
                }
              } catch {
                /* ignore poll errors */
              }
            }, 2000);
          });
        }

        if (!result.success) {
          // Clean up orphaned server on OAuth failure
          await onDeleteServer(serverId).catch(() => {});
          setCreatedServerId(null);
        }
        setTestResult(result);
      } else {
        const result = await onTest(buildConfig());
        setTestResult(result);
      }
    } catch (err) {
      setTestResult({ success: false, error: err instanceof Error ? err.message : "Test failed" });
    } finally {
      setTesting(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (createdServerId) {
      // Server was already created during OAuth connect flow
      onCancel();
      return;
    }
    setError(null);
    setSubmitting(true);
    try {
      await onSubmit({
        name,
        domain,
        description,
        visibility,
        config: buildConfig(),
        ...(authType === "oauth" && {
          oauthConfig: {
            clientId,
            authorizationUrl,
            tokenUrl,
            scopes: oauthScopes.split(/[\s,]+/).filter(Boolean),
          },
          clientSecret,
        }),
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create server");
    } finally {
      setSubmitting(false);
    }
  };

  const isValid = name.length > 0 && domain.length > 0 && (transport === "stdio" ? command.length > 0 : url.length > 0 && (authType !== "api-key" || apiKey.length > 0) && (authType !== "oauth" || (clientId.length > 0 && clientSecret.length > 0)));

  return (
    <form onSubmit={handleSubmit} className="border rounded-lg p-6 bg-white shadow-sm space-y-4">
      <h3 className="text-lg font-medium text-gray-900">Create Built-in Server</h3>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700">Name</label>
          <input type="text" value={name} onChange={(e) => setName(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="File System Server" required />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700">Domain</label>
          <input type="text" value={domain} onChange={(e) => setDomain(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="files" required />
        </div>
      </div>

      <div>
        <label className="block text-sm font-medium text-gray-700">Description</label>
        <textarea value={description} onChange={(e) => setDescription(e.target.value.slice(0, 500))} rows={2} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Provides file system access for reading and writing files" />
        <p className="mt-1 text-sm text-gray-500">{description.length}/500</p>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700">Visibility</label>
          <select value={visibility} onChange={(e) => setVisibility(e.target.value as "hidden" | "visible")} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
            <option value="visible">Visible (Users can toggle)</option>
            <option value="hidden">Hidden (System only)</option>
          </select>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700">Transport</label>
          <select value={transport} onChange={(e) => setTransport(e.target.value as TransportType)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
            <option value="http">HTTP (Remote)</option>
            <option value="stdio">Stdio (Local)</option>
          </select>
        </div>
      </div>

      {transport === "http" ? (
        <>
          <div>
            <label className="block text-sm font-medium text-gray-700">Server URL</label>
            <input type="url" value={url} onChange={(e) => setUrl(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="https://mcp.example.com" required />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Authentication</label>
            <select value={authType} onChange={(e) => setAuthType(e.target.value as AuthType)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
              <option value="none">None</option>
              <option value="api-key">API Key</option>
              <option value="oauth">OAuth</option>
            </select>
          </div>
          {authType === "api-key" && (
            <div>
              <label className="block text-sm font-medium text-gray-700">API Key</label>
              <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Enter API key" required />
            </div>
          )}
          {authType === "oauth" && (
            <>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700">Client ID</label>
                  <input type="text" value={clientId} onChange={(e) => setClientId(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Google OAuth Client ID" required />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700">Client Secret</label>
                  <input type="password" value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Google OAuth Client Secret" required />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700">Scopes</label>
                <input type="text" value={oauthScopes} onChange={(e) => setOauthScopes(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono" placeholder="openid email profile" />
              </div>
              <details className="text-sm text-gray-500">
                <summary className="cursor-pointer hover:text-gray-700">Advanced OAuth settings</summary>
                <div className="mt-2 space-y-3">
                  <div>
                    <label className="block text-sm font-medium text-gray-700">Authorization URL</label>
                    <input type="url" value={authorizationUrl} onChange={(e) => setAuthorizationUrl(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700">Token URL</label>
                    <input type="url" value={tokenUrl} onChange={(e) => setTokenUrl(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" />
                  </div>
                </div>
              </details>
            </>
          )}
        </>
      ) : (
        <>
          <div>
            <label className="block text-sm font-medium text-gray-700">Command</label>
            <input type="text" value={command} onChange={(e) => setCommand(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono" placeholder="npx @modelcontextprotocol/server-filesystem" required />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Arguments (space-separated)</label>
            <input type="text" value={args} onChange={(e) => setArgs(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono" placeholder="/path/to/allowed/directory" />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Environment Variables</label>
            <div className="space-y-2 mt-1">
              {envPairs.map((pair, idx) => (
                <div key={idx} className="flex gap-2">
                  <input
                    type="text"
                    value={pair.key}
                    onChange={(e) => {
                      const newPairs = [...envPairs];
                      newPairs[idx].key = e.target.value;
                      setEnvPairs(newPairs);
                    }}
                    className="flex-1 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
                    placeholder="KEY"
                  />
                  <input
                    type="text"
                    value={pair.value}
                    onChange={(e) => {
                      const newPairs = [...envPairs];
                      newPairs[idx].value = e.target.value;
                      setEnvPairs(newPairs);
                    }}
                    className="flex-1 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
                    placeholder="value"
                  />
                  <button type="button" onClick={() => setEnvPairs(envPairs.filter((_, i) => i !== idx))} className="px-2 py-1 text-red-500 hover:text-red-700">
                    ✕
                  </button>
                </div>
              ))}
              <button type="button" onClick={() => setEnvPairs([...envPairs, { key: "", value: "" }])} className="text-sm text-blue-600 hover:text-blue-800">
                + Add environment variable
              </button>
            </div>
          </div>
        </>
      )}

      {testResult && <div className={`p-3 rounded-md ${testResult.success ? "bg-green-50 text-green-800" : "bg-red-50 text-red-800"}`}>{testResult.success ? authType === "oauth" ? <p>&#10003; OAuth connected successfully!</p> : <p>&#10003; Connected successfully! Found {testResult.toolCount} tools.</p> : <p>&#10007; {testResult.error}</p>}</div>}

      {error && <div className="p-3 rounded-md bg-red-50 text-red-800">{error}</div>}

      <div className="flex gap-3 justify-end">
        <button
          type="button"
          onClick={async () => {
            if (createdServerId && !testResult?.success) {
              await onDeleteServer(createdServerId).catch(() => {});
            }
            onCancel();
          }}
          className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50"
        >
          Cancel
        </button>
        <button type="button" onClick={handleTest} disabled={!isValid || testing} className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50 disabled:opacity-50">
          {testing ? (authType === "oauth" ? "Connecting..." : "Testing...") : authType === "oauth" ? "Connect with OAuth" : "Test Connection"}
        </button>
        <button type="submit" disabled={!isValid || !testResult?.success || submitting} className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50">
          {createdServerId ? "Done" : submitting ? "Creating..." : "Create Server"}
        </button>
      </div>
    </form>
  );
}

// =============================================================================
// Edit Server Form
// =============================================================================

type ServerConfigType = { transport: "stdio"; command: string; args?: string[]; env?: Record<string, string> } | { transport: "http"; url: string; authType: AuthType; apiKey?: string };

function EditServerForm({ server, onSubmit, onTest, onCancel, onOAuthInitiate, onOAuthRevoke, authFetch }: { server: AdminServerView; onSubmit: (serverId: string, updates: { name?: string; description?: string; domain?: string; visibility?: "hidden" | "visible"; config?: ServerConfigType; oauthConfig?: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] }; clientSecret?: string }) => Promise<void>; onTest: (config: ServerConfigType, serverId?: string) => Promise<{ success: boolean; toolCount?: number; error?: string }>; onCancel: () => void; onOAuthInitiate: (serverId: string) => Promise<{ authUrl: string }>; onOAuthRevoke: (serverId: string) => Promise<void>; authFetch: (url: string, options?: RequestInit) => Promise<Response> }) {
  const cfg = server.config || {};
  const initialTransport = (cfg.transport as TransportType) || server.transport || "http";

  const [name, setName] = useState(server.name);
  const [domain, setDomain] = useState(server.domain);
  const [description, setDescription] = useState(server.description);
  const [visibility, setVisibility] = useState<"hidden" | "visible">(server.visibility === "hidden" ? "hidden" : "visible");
  const [transport, setTransport] = useState<TransportType>(initialTransport);

  // HTTP config
  const [url, setUrl] = useState((cfg.url as string) || "");
  const [authType, setAuthType] = useState<AuthType>((cfg.authType as AuthType) || server.authType || "none");
  const [apiKey, setApiKey] = useState("");

  // OAuth config
  const [clientId, setClientId] = useState(server.oauthConfig?.clientId || "");
  const [clientSecret, setClientSecret] = useState("");
  const [oauthScopes, setOauthScopes] = useState(server.oauthConfig?.scopes?.join(" ") || "openid email profile");
  const [authorizationUrl, setAuthorizationUrl] = useState(server.oauthConfig?.authorizationUrl || "https://accounts.google.com/o/oauth2/v2/auth");
  const [tokenUrl, setTokenUrl] = useState(server.oauthConfig?.tokenUrl || "https://oauth2.googleapis.com/token");
  const [oauthStatus, setOauthStatus] = useState<{ connected: boolean; expiresAt?: string } | null>(null);
  const [oauthLoading, setOauthLoading] = useState(false);

  // Stdio config
  const [command, setCommand] = useState((cfg.command as string) || "");
  const [args, setArgs] = useState(Array.isArray(cfg.args) ? (cfg.args as string[]).join(" ") : "");
  const [envPairs, setEnvPairs] = useState<{ key: string; value: string }[]>(() => {
    const env = cfg.env as Record<string, string> | undefined;
    if (env && typeof env === "object") {
      return Object.entries(env).map(([key, value]) => ({ key, value }));
    }
    return [];
  });

  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; toolCount?: number; error?: string } | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch OAuth status on mount for OAuth servers
  useEffect(() => {
    if (authType !== "oauth") return;
    const fetchStatus = async () => {
      try {
        const res = await authFetch(`/mcp/servers/${server.id}/oauth/status`);
        if (res.ok) {
          const data = await res.json();
          setOauthStatus(data);
        }
      } catch {
        // ignore — status is optional
      }
    };
    fetchStatus();
  }, [authType, server.id, authFetch]);

  const buildConfig = (): ServerConfigType => {
    if (transport === "stdio") {
      const env: Record<string, string> = {};
      for (const pair of envPairs) {
        if (pair.key) env[pair.key] = pair.value;
      }
      return {
        transport: "stdio" as const,
        command,
        args: args ? args.split(/\s+/) : undefined,
        env: Object.keys(env).length > 0 ? env : undefined,
      };
    } else {
      return {
        transport: "http" as const,
        url,
        authType,
        apiKey: authType === "api-key" ? apiKey : undefined,
      };
    }
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await onTest(buildConfig(), server.id);
      setTestResult(result);
    } catch {
      setTestResult({ success: false, error: "Test failed" });
    } finally {
      setTesting(false);
    }
  };

  const handleOAuthReauthorize = async () => {
    setOauthLoading(true);
    setError(null);
    try {
      const { authUrl } = await onOAuthInitiate(server.id);
      const popup = await openExternal(authUrl, "oauth-popup", "width=600,height=700");

      let result: { success: boolean; error?: string };

      if (popup) {
        // Browser: listen for postMessage from popup callback
        result = await new Promise<{ success: boolean; error?: string }>((resolve) => {
          let resolved = false;
          const cleanup = () => {
            resolved = true;
            window.removeEventListener("message", listener);
            clearTimeout(timeout);
            clearInterval(pollClosed);
          };
          const listener = (event: MessageEvent) => {
            if (resolved) return;
            if (event.data?.type === "oauth-success" && event.data.serverId === server.id) {
              cleanup();
              resolve({ success: true });
            } else if (event.data?.type === "oauth-error") {
              cleanup();
              resolve({ success: false, error: event.data.error || "OAuth authorization failed" });
            }
          };
          window.addEventListener("message", listener);
          const timeout = setTimeout(() => {
            cleanup();
            if (popup && !popup.closed) popup.close();
            resolve({ success: false, error: "OAuth authorization timed out" });
          }, 120000);
          const pollClosed = setInterval(() => {
            if (popup && popup.closed) {
              cleanup();
              resolve({ success: false, error: "OAuth window was closed" });
            }
          }, 500);
        });
      } else {
        // Tauri / no popup: poll OAuth status endpoint until connected
        result = await new Promise<{ success: boolean; error?: string }>((resolve) => {
          let stopped = false;
          const timeout = setTimeout(() => {
            stopped = true;
            clearInterval(poll);
            resolve({ success: false, error: "OAuth authorization timed out" });
          }, 120000);
          const poll = setInterval(async () => {
            if (stopped) return;
            try {
              const res = await authFetch(`/mcp/servers/${server.id}/oauth/status`);
              if (res.ok) {
                const status = await res.json();
                if (status.connected) {
                  stopped = true;
                  clearTimeout(timeout);
                  clearInterval(poll);
                  resolve({ success: true });
                }
              }
            } catch {
              /* ignore poll errors */
            }
          }, 2000);
        });
      }

      if (result.success) {
        // Refresh status
        const res = await authFetch(`/mcp/servers/${server.id}/oauth/status`);
        if (res.ok) setOauthStatus(await res.json());
      } else {
        setError(result.error || "OAuth failed");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "OAuth initiation failed");
    } finally {
      setOauthLoading(false);
    }
  };

  const handleOAuthDisconnect = async () => {
    if (!confirm("Disconnect OAuth? You will need to re-authorize to use this server.")) return;
    setOauthLoading(true);
    try {
      await onOAuthRevoke(server.id);
      setOauthStatus({ connected: false });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to revoke OAuth");
    } finally {
      setOauthLoading(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await onSubmit(server.id, {
        name,
        description,
        domain,
        visibility,
        config: buildConfig(),
        ...(authType === "oauth" && {
          oauthConfig: {
            clientId,
            authorizationUrl,
            tokenUrl,
            scopes: oauthScopes.split(/[\s,]+/).filter(Boolean),
          },
          ...(clientSecret ? { clientSecret } : {}),
        }),
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update server");
    } finally {
      setSubmitting(false);
    }
  };

  const isValid = name.length > 0 && domain.length > 0 && (transport === "stdio" ? command.length > 0 : url.length > 0 && (authType !== "api-key" || apiKey.length > 0) && (authType !== "oauth" || clientId.length > 0));

  return (
    <form onSubmit={handleSubmit} className="border rounded-lg p-6 bg-white shadow-sm space-y-4">
      <h3 className="text-lg font-medium text-gray-900">Edit Server: {server.name}</h3>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700">Name</label>
          <input type="text" value={name} onChange={(e) => setName(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" required />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700">Domain</label>
          <input type="text" value={domain} onChange={(e) => setDomain(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" required />
        </div>
      </div>

      <div>
        <label className="block text-sm font-medium text-gray-700">Description</label>
        <textarea value={description} onChange={(e) => setDescription(e.target.value.slice(0, 500))} rows={2} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" />
        <p className="mt-1 text-sm text-gray-500">{description.length}/500</p>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700">Visibility</label>
          <select value={visibility} onChange={(e) => setVisibility(e.target.value as "hidden" | "visible")} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
            <option value="visible">Visible (Users can toggle)</option>
            <option value="hidden">Hidden (System only)</option>
          </select>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700">Transport</label>
          <select value={transport} onChange={(e) => setTransport(e.target.value as TransportType)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
            <option value="http">HTTP (Remote)</option>
            <option value="stdio">Stdio (Local)</option>
          </select>
        </div>
      </div>

      {transport === "http" ? (
        <>
          <div>
            <label className="block text-sm font-medium text-gray-700">Server URL</label>
            <input type="url" value={url} onChange={(e) => setUrl(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="https://mcp.example.com" required />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Authentication</label>
            <select value={authType} onChange={(e) => setAuthType(e.target.value as AuthType)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
              <option value="none">None</option>
              <option value="api-key">API Key</option>
              <option value="oauth">OAuth</option>
            </select>
          </div>
          {authType === "api-key" && (
            <div>
              <label className="block text-sm font-medium text-gray-700">API Key</label>
              <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Enter new API key (leave blank to keep existing)" />
            </div>
          )}
          {authType === "oauth" && (
            <>
              {oauthStatus && (
                <div className={`p-3 rounded-md ${oauthStatus.connected ? "bg-green-50 border border-green-200" : "bg-yellow-50 border border-yellow-200"}`}>
                  <div className="flex items-center justify-between">
                    <div>
                      <p className={`text-sm font-medium ${oauthStatus.connected ? "text-green-800" : "text-yellow-800"}`}>{oauthStatus.connected ? "\u2713 OAuth Connected" : "\u26A0 OAuth Not Connected"}</p>
                      {oauthStatus.expiresAt && <p className="text-xs text-gray-500 mt-1">Token expires: {new Date(oauthStatus.expiresAt).toLocaleString()}</p>}
                    </div>
                    <div className="flex gap-2">
                      <button type="button" onClick={handleOAuthReauthorize} disabled={oauthLoading} className="px-3 py-1 text-xs font-medium text-blue-700 bg-blue-100 rounded-md hover:bg-blue-200 disabled:opacity-50">
                        {oauthLoading ? "..." : "Re-authorize"}
                      </button>
                      {oauthStatus.connected && (
                        <button type="button" onClick={handleOAuthDisconnect} disabled={oauthLoading} className="px-3 py-1 text-xs font-medium text-red-700 bg-red-100 rounded-md hover:bg-red-200 disabled:opacity-50">
                          Disconnect
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              )}
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700">Client ID</label>
                  <input type="text" value={clientId} onChange={(e) => setClientId(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Google OAuth Client ID" required />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700">Client Secret</label>
                  <input type="password" value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Leave blank to keep existing" />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700">Scopes</label>
                <input type="text" value={oauthScopes} onChange={(e) => setOauthScopes(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono" placeholder="openid email profile" />
              </div>
              <details className="text-sm text-gray-500">
                <summary className="cursor-pointer hover:text-gray-700">Advanced OAuth settings</summary>
                <div className="mt-2 space-y-3">
                  <div>
                    <label className="block text-sm font-medium text-gray-700">Authorization URL</label>
                    <input type="url" value={authorizationUrl} onChange={(e) => setAuthorizationUrl(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700">Token URL</label>
                    <input type="url" value={tokenUrl} onChange={(e) => setTokenUrl(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" />
                  </div>
                </div>
              </details>
            </>
          )}
        </>
      ) : (
        <>
          <div>
            <label className="block text-sm font-medium text-gray-700">Command</label>
            <input type="text" value={command} onChange={(e) => setCommand(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono" required />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Arguments (space-separated)</label>
            <input type="text" value={args} onChange={(e) => setArgs(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono" />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Environment Variables</label>
            <div className="space-y-2 mt-1">
              {envPairs.map((pair, idx) => (
                <div key={idx} className="flex gap-2">
                  <input
                    type="text"
                    value={pair.key}
                    onChange={(e) => {
                      const newPairs = [...envPairs];
                      newPairs[idx].key = e.target.value;
                      setEnvPairs(newPairs);
                    }}
                    className="flex-1 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
                    placeholder="KEY"
                  />
                  <input
                    type="text"
                    value={pair.value}
                    onChange={(e) => {
                      const newPairs = [...envPairs];
                      newPairs[idx].value = e.target.value;
                      setEnvPairs(newPairs);
                    }}
                    className="flex-1 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
                    placeholder="value"
                  />
                  <button type="button" onClick={() => setEnvPairs(envPairs.filter((_, i) => i !== idx))} className="px-2 py-1 text-red-500 hover:text-red-700">
                    ✕
                  </button>
                </div>
              ))}
              <button type="button" onClick={() => setEnvPairs([...envPairs, { key: "", value: "" }])} className="text-sm text-blue-600 hover:text-blue-800">
                + Add environment variable
              </button>
            </div>
          </div>
        </>
      )}

      {testResult && <div className={`p-3 rounded-md ${testResult.success ? "bg-green-50 text-green-800" : "bg-red-50 text-red-800"}`}>{testResult.success ? <p>&#10003; Connected successfully! Found {testResult.toolCount} tools.</p> : <p>&#10007; {testResult.error}</p>}</div>}

      {error && <div className="p-3 rounded-md bg-red-50 text-red-800">{error}</div>}

      <div className="flex gap-3 justify-end">
        <button type="button" onClick={onCancel} className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50">
          Cancel
        </button>
        <button type="button" onClick={handleTest} disabled={!isValid || testing} className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50 disabled:opacity-50">
          {testing ? "Testing..." : "Test Connection"}
        </button>
        <button type="submit" disabled={!isValid || submitting} className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50">
          {submitting ? "Saving..." : "Save Changes"}
        </button>
      </div>
    </form>
  );
}

// =============================================================================
// Main Page
// =============================================================================

export default function AdminToolsPage() {
  const { isLoading: authLoading, isAuthenticated } = useAuth();
  const authFetch = useAuthFetch();

  const [servers, setServers] = useState<AdminServerView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [editingServerId, setEditingServerId] = useState<string | null>(null);
  const [groupBy, setGroupBy] = useState<"visibility" | "transport">("visibility");

  const loadServers = useCallback(async () => {
    try {
      const res = await authFetch("/mcp/admin/servers");
      if (res.ok) {
        const data = await res.json();
        setServers(data.servers || []);
      } else if (res.status === 403) {
        setError("Admin access required");
      } else {
        setError("Failed to load servers");
      }
    } catch (err) {
      setError("Failed to load servers");
      console.error(err);
    } finally {
      setLoading(false);
    }
  }, [authFetch]);

  useEffect(() => {
    if (authLoading) return;
    if (!isAuthenticated) return;
    loadServers();
  }, [authLoading, isAuthenticated, loadServers]);

  const handleToggleEnabled = async (serverId: string, enabled: boolean) => {
    try {
      const res = await authFetch(`/mcp/admin/servers/${serverId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ enabled }),
      });
      if (res.ok) {
        setServers((prev) => prev.map((s) => (s.id === serverId ? { ...s, status: enabled ? "enabled" : "disabled" } : s)));
      }
    } catch (err) {
      console.error("Failed to toggle server", err);
    }
  };

  const handleDelete = async (serverId: string) => {
    const server = servers.find((s) => s.id === serverId);
    if (!server) return;

    const hasUsers = server.userPreferenceCount > 0;
    const message = hasUsers ? `This server has ${server.userPreferenceCount} users with it enabled. Delete anyway?` : "Are you sure you want to delete this server?";

    if (!confirm(message)) return;

    try {
      const res = await authFetch(`/mcp/admin/servers/${serverId}`, { method: "DELETE" });
      if (res.ok) {
        setServers((prev) => prev.filter((s) => s.id !== serverId));
      }
    } catch (err) {
      console.error("Failed to delete server", err);
    }
  };

  const handleCreate = async (config: { name: string; domain: string; description: string; visibility: "hidden" | "visible"; config: { transport: "stdio"; command: string; args?: string[]; env?: Record<string, string> } | { transport: "http"; url: string; authType: AuthType; apiKey?: string }; oauthConfig?: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] }; clientSecret?: string }) => {
    const res = await authFetch("/mcp/admin/servers", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(config),
    });
    if (!res.ok) {
      const data = await res.json();
      throw new Error(data.message || "Failed to create server");
    }
    setShowCreateForm(false);
    await loadServers();
  };

  const handleTest = async (config: { transport: "stdio"; command: string; args?: string[]; env?: Record<string, string> } | { transport: "http"; url: string; authType: AuthType; apiKey?: string }, serverId?: string) => {
    const res = await authFetch("/mcp/test-connection", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ config, serverId }),
    });
    return res.json();
  };

  const handleOAuthConnect = async (formData: { name: string; domain: string; description: string; visibility: "hidden" | "visible"; config: { transport: "http"; url: string; authType: "oauth" }; oauthConfig: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] }; clientSecret: string }) => {
    // Step 1: Create the server
    const createRes = await authFetch("/mcp/admin/servers", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(formData),
    });
    if (!createRes.ok) {
      const data = await createRes.json();
      throw new Error(data.message || "Failed to create server");
    }
    const server = await createRes.json();

    // Step 2: Initiate OAuth
    const oauthRes = await authFetch(`/mcp/servers/${server.id}/oauth/initiate`, {
      method: "POST",
    });
    if (!oauthRes.ok) {
      // Clean up the server we just created
      await authFetch(`/mcp/admin/servers/${server.id}`, { method: "DELETE" });
      const data = await oauthRes.json().catch(() => ({}));
      throw new Error(data.message || "Failed to initiate OAuth");
    }
    const { authUrl } = await oauthRes.json();

    // Refresh server list in background
    loadServers();

    return { serverId: server.id, authUrl };
  };

  const handleDeleteServer = async (serverId: string) => {
    await authFetch(`/mcp/admin/servers/${serverId}`, { method: "DELETE" });
    await loadServers();
  };

  const handleOAuthInitiateExisting = async (serverId: string) => {
    const oauthRes = await authFetch(`/mcp/servers/${serverId}/oauth/initiate`, {
      method: "POST",
    });
    if (!oauthRes.ok) {
      const data = await oauthRes.json().catch(() => ({}));
      throw new Error(data.message || "Failed to initiate OAuth");
    }
    const { authUrl } = await oauthRes.json();
    return { authUrl };
  };

  const handleOAuthRevoke = async (serverId: string) => {
    const res = await authFetch(`/mcp/servers/${serverId}/oauth/revoke`, {
      method: "POST",
    });
    if (!res.ok) {
      const data = await res.json().catch(() => ({}));
      throw new Error(data.message || "Failed to revoke OAuth");
    }
  };

  const handleEdit = (serverId: string) => {
    setEditingServerId(serverId);
    setShowCreateForm(false);
  };

  const handleUpdate = async (serverId: string, updates: { name?: string; description?: string; domain?: string; visibility?: "hidden" | "visible"; config?: ServerConfigType; oauthConfig?: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] }; clientSecret?: string }) => {
    const res = await authFetch(`/mcp/admin/servers/${serverId}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(updates),
    });
    if (!res.ok) {
      const data = await res.json();
      throw new Error(data.message || "Failed to update server");
    }
    setEditingServerId(null);
    await loadServers();
  };

  if (authLoading || loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
      </div>
    );
  }

  return (
    <div>
      <div className="mb-6">
        <h2 className="text-xl font-bold text-gray-900">MCP Server Administration</h2>
        <p className="text-gray-600 mt-1">Manage system-wide MCP server configurations</p>
      </div>

      {error && <div className="mb-4 p-3 bg-red-50 text-red-800 rounded-md">{error}</div>}

      <div className="mb-6 flex items-center justify-between">
        <div className="flex items-center gap-4">
          <label className="text-sm font-medium text-gray-700">Group by:</label>
          <select value={groupBy} onChange={(e) => setGroupBy(e.target.value as "visibility" | "transport")} className="rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
            <option value="visibility">Visibility</option>
            <option value="transport">Transport</option>
          </select>
        </div>
        <button
          onClick={() => {
            setShowCreateForm(!showCreateForm);
            setEditingServerId(null);
          }}
          className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
        >
          {showCreateForm ? "Cancel" : "Create Server"}
        </button>
      </div>

      {showCreateForm && (
        <div className="mb-6">
          <CreateServerForm onSubmit={handleCreate} onTest={handleTest} onOAuthConnect={handleOAuthConnect} onDeleteServer={handleDeleteServer} onCancel={() => setShowCreateForm(false)} authFetch={authFetch} />
        </div>
      )}

      {editingServerId &&
        (() => {
          const editingServer = servers.find((s) => s.id === editingServerId);
          if (!editingServer) return null;
          return (
            <div className="mb-6">
              <EditServerForm server={editingServer} onSubmit={handleUpdate} onTest={handleTest} onCancel={() => setEditingServerId(null)} onOAuthInitiate={handleOAuthInitiateExisting} onOAuthRevoke={handleOAuthRevoke} authFetch={authFetch} />
            </div>
          );
        })()}

      {servers.length === 0 ? (
        <div className="text-center py-12 bg-white rounded-lg border">
          <p className="text-gray-500">No MCP servers configured.</p>
          <p className="text-sm text-gray-400 mt-1">Create a server to get started.</p>
        </div>
      ) : (
        <AdminServerList servers={servers} groupBy={groupBy} onEdit={handleEdit} onDelete={handleDelete} onToggleEnabled={handleToggleEnabled} />
      )}
    </div>
  );
}
