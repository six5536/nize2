// @zen-component: PLAN-020-UserToolsUI

/**
 * User MCP server management page at /settings/tools
 *
 * Allows users to view, add, toggle, and delete their MCP server connections.
 */

"use client";

import { useEffect, useState, useCallback } from "react";
import { useAuth, useAuthFetch } from "@/lib/auth-context";
import { ServerForm, type ServerConfig, type TestConnectionResult } from "@/components/mcp-server";

// =============================================================================
// Types
// =============================================================================

type ServerStatus = "enabled" | "disabled" | "unavailable" | "unauthorized";

interface UserServerView {
  id: string;
  name: string;
  description: string;
  domain: string;
  visibility: "visible" | "user";
  status: ServerStatus;
  toolCount: number;
  isOwned: boolean;
}

interface ServerTool {
  name: string;
  description: string;
}

// =============================================================================
// Components
// =============================================================================

function ServerListItem({ server, onToggle, onExpand, onDelete, onEdit, isExpanded, tools }: { server: UserServerView; onToggle: (enabled: boolean) => void; onExpand: () => void; onDelete?: () => void; onEdit?: () => void; isExpanded: boolean; tools: ServerTool[] }) {
  const statusColors: Record<ServerStatus, string> = {
    enabled: "bg-green-100 text-green-800",
    disabled: "bg-gray-100 text-gray-800",
    unavailable: "bg-red-100 text-red-800",
    unauthorized: "bg-yellow-100 text-yellow-800",
  };

  const statusLabels: Record<ServerStatus, string> = {
    enabled: "Enabled",
    disabled: "Disabled",
    unavailable: "Unavailable",
    unauthorized: "Needs Auth",
  };

  return (
    <div className="border rounded-lg p-4 bg-white shadow-sm">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <button onClick={onExpand} className="text-gray-500 hover:text-gray-700" aria-label={isExpanded ? "Collapse" : "Expand"}>
            <svg className={`w-5 h-5 transition-transform ${isExpanded ? "rotate-90" : ""}`} fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
            </svg>
          </button>
          <div>
            <h3 className="font-medium text-gray-900">{server.name}</h3>
            <p className="text-sm text-gray-500">{server.domain}</p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <span className={`px-2 py-1 text-xs font-medium rounded-full ${statusColors[server.status]}`}>{statusLabels[server.status]}</span>
          <span className="text-sm text-gray-500">{server.toolCount} tools</span>

          {server.status !== "unavailable" && server.status !== "unauthorized" && (
            <label className="relative inline-flex items-center cursor-pointer">
              <input type="checkbox" className="sr-only peer" checked={server.status === "enabled"} onChange={(e) => onToggle(e.target.checked)} />
              <div className="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
            </label>
          )}

          {server.isOwned && (
            <button onClick={onEdit} className="p-1 text-gray-500 hover:text-gray-700" aria-label="Edit server">
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
            </button>
          )}

          {server.isOwned && (
            <button onClick={onDelete} className="p-1 text-red-500 hover:text-red-700" aria-label="Delete server">
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          )}
        </div>
      </div>

      {isExpanded && (
        <div className="mt-4 pl-8 border-t pt-4">
          {server.description && <p className="text-sm text-gray-600 mb-3">{server.description}</p>}
          <h4 className="text-sm font-medium text-gray-700 mb-2">Available Tools</h4>
          {tools.length === 0 ? (
            <p className="text-sm text-gray-500 italic">No tools available</p>
          ) : (
            <ul className="space-y-2">
              {tools.map((tool) => (
                <li key={tool.name} className="text-sm">
                  <span className="font-mono text-blue-600">{tool.name}</span>
                  <p className="text-gray-500 text-xs">{tool.description}</p>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}

// =============================================================================
// Main Page
// =============================================================================

export default function UserToolsPage() {
  const { isLoading: authLoading, isAuthenticated } = useAuth();
  const authFetch = useAuthFetch();

  const [servers, setServers] = useState<UserServerView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showAddForm, setShowAddForm] = useState(false);
  const [editingServerId, setEditingServerId] = useState<string | null>(null);
  const [expandedServerId, setExpandedServerId] = useState<string | null>(null);
  const [serverTools, setServerTools] = useState<Record<string, ServerTool[]>>({});

  const loadServers = useCallback(async () => {
    try {
      const res = await authFetch("/mcp/servers");
      if (res.ok) {
        const data = await res.json();
        setServers(data.servers || []);
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

  const handleToggle = async (serverId: string, enabled: boolean) => {
    try {
      const res = await authFetch(`/mcp/servers/${serverId}/preference`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ enabled }),
      });
      if (res.ok) {
        setServers((prev) => prev.map((s) => (s.id === serverId ? { ...s, status: enabled ? "enabled" : "disabled" } : s)));
      }
    } catch (err) {
      console.error("Failed to toggle preference", err);
    }
  };

  const handleExpand = async (serverId: string) => {
    if (expandedServerId === serverId) {
      setExpandedServerId(null);
      return;
    }

    setExpandedServerId(serverId);

    if (!serverTools[serverId]) {
      try {
        const res = await authFetch(`/mcp/servers/${serverId}/tools`);
        if (res.ok) {
          const data = await res.json();
          setServerTools((prev) => ({ ...prev, [serverId]: data.tools || [] }));
        }
      } catch (err) {
        console.error("Failed to load tools", err);
      }
    }
  };

  const handleDelete = async (serverId: string) => {
    if (!confirm("Are you sure you want to delete this server?")) return;
    try {
      const res = await authFetch(`/mcp/servers/${serverId}`, { method: "DELETE" });
      if (res.ok) {
        setServers((prev) => prev.filter((s) => s.id !== serverId));
      }
    } catch (err) {
      console.error("Failed to delete server", err);
    }
  };

  // @zen-impl: PLAN-032 Step 10 — ServerForm callbacks for user page
  // User API uses flat fields (url, authType, apiKey), not nested config objects
  const handleTestConnection = async (config: ServerConfig, serverId?: string): Promise<TestConnectionResult> => {
    const res = await authFetch("/mcp/test-connection", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ config, serverId }),
    });
    return res.json();
  };

  const handleCreateServer = async (payload: { name: string; description: string; domain: string; visibility: "hidden" | "visible"; config: ServerConfig; oauthConfig?: { clientId: string; authorizationUrl: string; tokenUrl: string; scopes: string[] }; clientSecret?: string }): Promise<string> => {
    // Flatten config for user API
    const httpConfig = payload.config as { transport: "http" | "sse"; url: string; authType: string; apiKey?: string };
    const body: Record<string, unknown> = {
      name: payload.name,
      description: payload.description,
      domain: payload.domain,
      // @zen-impl: XMCP-5_AC-1 — send transport type to user API
      transport: httpConfig.transport,
      url: httpConfig.url,
      authType: httpConfig.authType,
    };
    if (httpConfig.apiKey) body.apiKey = httpConfig.apiKey;
    if (payload.oauthConfig) body.oauthConfig = payload.oauthConfig;
    if (payload.clientSecret) body.clientSecret = payload.clientSecret;

    const res = await authFetch("/mcp/servers", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const data = await res.json();
      throw new Error(data.message || "Failed to add server");
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
    // Flatten config for user API
    const body: Record<string, unknown> = {};
    if (updates.name !== undefined) body.name = updates.name;
    if (updates.description !== undefined) body.description = updates.description;
    if (updates.domain !== undefined) body.domain = updates.domain;
    if (updates.config) {
      const httpConfig = updates.config as { transport: "http" | "sse"; url: string; authType: string; apiKey?: string };
      if (httpConfig.transport) body.transport = httpConfig.transport;
      if (httpConfig.url) body.url = httpConfig.url;
      if (httpConfig.authType) body.authType = httpConfig.authType;
      if (httpConfig.apiKey) body.apiKey = httpConfig.apiKey;
    }

    const res = await authFetch(`/mcp/servers/${serverId}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const data = await res.json();
      throw new Error(data.message || "Failed to update server");
    }
  };

  const handleDeleteServer = async (serverId: string) => {
    await authFetch(`/mcp/servers/${serverId}`, { method: "DELETE" });
  };

  const handleEdit = (serverId: string) => {
    setEditingServerId(serverId);
    setShowAddForm(false);
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
      <p className="text-gray-600 mb-6">Manage your MCP server connections and tool preferences</p>

      {error && <div className="mb-4 p-3 bg-red-50 text-red-800 rounded-md">{error}</div>}

      <div className="mb-6 flex justify-end">
        <button
          onClick={() => {
            setShowAddForm(!showAddForm);
            setEditingServerId(null);
          }}
          className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
        >
          {showAddForm ? "Cancel" : "Add Server"}
        </button>
      </div>

      {showAddForm && (
        <div className="mb-6">
          <ServerForm
            mode="create"
            showTransport
            transportOptions={["http", "sse"]}
            authFetch={authFetch}
            onTestConnection={handleTestConnection}
            onCreateServer={handleCreateServer}
            onDeleteServer={handleDeleteServer}
            onCancel={() => setShowAddForm(false)}
            onSuccess={() => {
              setShowAddForm(false);
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
                  visibility: "visible",
                  transport: "http",
                  authType: "none",
                }}
                showTransport
                transportOptions={["http", "sse"]}
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

      <div className="space-y-4">
        {servers.length === 0 ? (
          <div className="text-center py-12 bg-white rounded-lg border">
            <p className="text-gray-500">No MCP servers configured.</p>
            <p className="text-sm text-gray-400 mt-1">Add a server to get started.</p>
          </div>
        ) : (
          servers.map((server) => <ServerListItem key={server.id} server={server} onToggle={(enabled) => handleToggle(server.id, enabled)} onExpand={() => handleExpand(server.id)} onDelete={server.isOwned ? () => handleDelete(server.id) : undefined} onEdit={server.isOwned ? () => handleEdit(server.id) : undefined} isExpanded={expandedServerId === server.id} tools={serverTools[server.id] || []} />)
        )}
      </div>
    </div>
  );
}
