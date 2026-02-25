// @awa-component: PLAN-020-AdminToolsUI

/**
 * Admin MCP server management page at /settings/admin/tools
 *
 * Allows admins to view, create, edit, toggle, and delete
 * built-in MCP server configurations system-wide.
 */

"use client";

import { useEffect, useState, useCallback } from "react";
import { useAuth, useAuthFetch } from "@/lib/auth-context";
import { ServerForm, type ServerConfig, type TestConnectionResult } from "@/components/mcp-server";

// =============================================================================
// Types
// =============================================================================

type ServerStatus = "enabled" | "disabled" | "unavailable" | "unauthorized";
type VisibilityTier = "hidden" | "visible" | "user";
type TransportType = "stdio" | "http" | "sse" | "managed-sse" | "managed-http";
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
    sse: "SSE (Remote)",
    "managed-sse": "Managed SSE (Local)",
    "managed-http": "Managed HTTP (Local)",
  };

  const labels = groupBy === "visibility" ? visibilityLabels : transportLabels;
  const order = groupBy === "visibility" ? ["hidden", "visible", "user"] : ["stdio", "http", "sse", "managed-sse", "managed-http"];

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

  // @awa-impl: PLAN-032 Step 9 — ServerForm callbacks for admin page
  const handleTestConnection = async (config: ServerConfig, serverId?: string): Promise<TestConnectionResult> => {
    const res = await authFetch("/mcp/test-connection", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ config, serverId }),
    });
    return res.json();
  };

  const handleCreateServer = async (payload: { name: string; description: string; domain: string; visibility: "hidden" | "visible"; config: ServerConfig; oauthConfig?: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] }; clientSecret?: string }): Promise<string> => {
    const res = await authFetch("/mcp/admin/servers", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (!res.ok) {
      const data = await res.json();
      throw new Error(data.message || "Failed to create server");
    }
    const server = await res.json();
    loadServers();
    return server.id;
  };

  const handleUpdateServer = async (
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
  ): Promise<void> => {
    const res = await authFetch(`/mcp/admin/servers/${serverId}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(updates),
    });
    if (!res.ok) {
      const data = await res.json();
      throw new Error(data.message || "Failed to update server");
    }
  };

  const handleDeleteServer = async (serverId: string) => {
    await authFetch(`/mcp/admin/servers/${serverId}`, { method: "DELETE" });
  };

  const handleEdit = (serverId: string) => {
    setEditingServerId(serverId);
    setShowCreateForm(false);
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
          <ServerForm
            mode="create"
            showTransport
            showVisibility
            authFetch={authFetch}
            onTestConnection={handleTestConnection}
            onCreateServer={handleCreateServer}
            onDeleteServer={handleDeleteServer}
            onCancel={() => setShowCreateForm(false)}
            onSuccess={() => {
              setShowCreateForm(false);
              loadServers();
            }}
          />
        </div>
      )}

      {editingServerId &&
        (() => {
          const editingServer = servers.find((s) => s.id === editingServerId);
          if (!editingServer) return null;
          return (
            <div className="mb-6">
              <ServerForm
                mode="edit"
                initialValues={{
                  id: editingServer.id,
                  name: editingServer.name,
                  description: editingServer.description,
                  domain: editingServer.domain,
                  visibility: editingServer.visibility,
                  transport: editingServer.transport,
                  authType: editingServer.authType,
                  config: editingServer.config,
                  oauthConfig: editingServer.oauthConfig,
                }}
                showTransport
                showVisibility
                authFetch={authFetch}
                onTestConnection={handleTestConnection}
                onUpdateServer={handleUpdateServer}
                onCancel={() => setEditingServerId(null)}
                onSuccess={() => {
                  setEditingServerId(null);
                  loadServers();
                }}
              />
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
