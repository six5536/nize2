// @zen-component: PLAN-020-UserToolsUI

/**
 * User MCP server management page at /settings/tools
 *
 * Allows users to view, add, toggle, and delete their MCP server connections.
 */

"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { useAuth, useAuthFetch } from "@/lib/auth-context";

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

function ServerListItem({ server, onToggle, onExpand, onDelete, isExpanded, tools }: { server: UserServerView; onToggle: (enabled: boolean) => void; onExpand: () => void; onDelete?: () => void; isExpanded: boolean; tools: ServerTool[] }) {
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

function AddServerForm({ onSubmit, onTest, onCancel }: { onSubmit: (config: { name: string; description?: string; domain: string; url: string; authType: string; apiKey?: string }) => Promise<void>; onTest: (config: { url: string; transport: string; authType: string; apiKey?: string }) => Promise<{ success: boolean; toolCount?: number; error?: string }>; onCancel: () => void }) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [domain, setDomain] = useState("");
  const [url, setUrl] = useState("");
  const [authType, setAuthType] = useState<"none" | "api-key" | "oauth">("none");
  const [apiKey, setApiKey] = useState("");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; toolCount?: number; error?: string } | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await onTest({
        transport: "http",
        url,
        authType,
        apiKey: authType === "api-key" ? apiKey : undefined,
      });
      setTestResult(result);
    } catch {
      setTestResult({ success: false, error: "Test failed" });
    } finally {
      setTesting(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await onSubmit({
        name,
        description: description || undefined,
        domain,
        url,
        authType,
        apiKey: authType === "api-key" ? apiKey : undefined,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to add server");
    } finally {
      setSubmitting(false);
    }
  };

  const isValid = name.length > 0 && domain.length > 0 && url.length > 0 && (authType !== "api-key" || apiKey.length > 0);

  return (
    <form onSubmit={handleSubmit} className="border rounded-lg p-6 bg-white shadow-sm space-y-4">
      <h3 className="text-lg font-medium text-gray-900">Add MCP Server</h3>

      <div>
        <label className="block text-sm font-medium text-gray-700">Name</label>
        <input type="text" value={name} onChange={(e) => setName(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="My Server" required />
      </div>

      <div>
        <label className="block text-sm font-medium text-gray-700">Description</label>
        <textarea value={description} onChange={(e) => setDescription(e.target.value)} maxLength={500} rows={2} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Describe what this server provides" />
        <p className="mt-1 text-xs text-gray-500">{description.length}/500 characters</p>
      </div>

      <div>
        <label className="block text-sm font-medium text-gray-700">Domain</label>
        <input type="text" value={domain} onChange={(e) => setDomain(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="e.g., files, search, code" required />
        <p className="mt-1 text-xs text-gray-500">Category for grouping tools</p>
      </div>

      <div>
        <label className="block text-sm font-medium text-gray-700">Server URL</label>
        <input type="url" value={url} onChange={(e) => setUrl(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="https://mcp.example.com" required />
      </div>

      <div>
        <label className="block text-sm font-medium text-gray-700">Authentication</label>
        <select value={authType} onChange={(e) => setAuthType(e.target.value as "none" | "api-key" | "oauth")} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
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

      {authType === "oauth" && <p className="text-sm text-yellow-600">OAuth configuration will be set up after connection test.</p>}

      {testResult && <div className={`p-3 rounded-md ${testResult.success ? "bg-green-50 text-green-800" : "bg-red-50 text-red-800"}`}>{testResult.success ? <p>&#10003; Connected successfully! Found {testResult.toolCount} tools.</p> : <p>&#10007; {testResult.error}</p>}</div>}

      {error && <div className="p-3 rounded-md bg-red-50 text-red-800">{error}</div>}

      <div className="flex gap-3 justify-end">
        <button type="button" onClick={onCancel} className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50">
          Cancel
        </button>
        <button type="button" onClick={handleTest} disabled={!url || testing} className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50 disabled:opacity-50">
          {testing ? "Testing..." : "Test Connection"}
        </button>
        <button type="submit" disabled={!isValid || !testResult?.success || submitting} className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50">
          {submitting ? "Adding..." : "Add Server"}
        </button>
      </div>
    </form>
  );
}

// =============================================================================
// Main Page
// =============================================================================

export default function UserToolsPage() {
  const { isLoading: authLoading, isAuthenticated } = useAuth();
  const authFetch = useAuthFetch();
  const router = useRouter();

  const [servers, setServers] = useState<UserServerView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showAddForm, setShowAddForm] = useState(false);
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
    if (!isAuthenticated) {
      router.replace("/login");
      return;
    }
    loadServers();
  }, [authLoading, isAuthenticated, router, loadServers]);

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

  const handleAdd = async (config: { name: string; description?: string; domain: string; url: string; authType: string; apiKey?: string }) => {
    const res = await authFetch("/mcp/servers", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(config),
    });
    if (!res.ok) {
      const data = await res.json();
      throw new Error(data.message || "Failed to add server");
    }
    setShowAddForm(false);
    await loadServers();
  };

  const handleTest = async (config: { url: string; transport: string; authType: string; apiKey?: string }) => {
    const res = await authFetch("/mcp/test-connection", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(config),
    });
    return res.json();
  };

  if (authLoading || loading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-50 py-8">
      <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="mb-8">
          <h1 className="text-2xl font-bold text-gray-900">MCP Tools</h1>
          <p className="text-gray-600 mt-1">Manage your MCP server connections and tool preferences</p>
        </div>

        {error && <div className="mb-4 p-3 bg-red-50 text-red-800 rounded-md">{error}</div>}

        <div className="mb-6 flex justify-end">
          <button onClick={() => setShowAddForm(!showAddForm)} className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700">
            {showAddForm ? "Cancel" : "Add Server"}
          </button>
        </div>

        {showAddForm && (
          <div className="mb-6">
            <AddServerForm onSubmit={handleAdd} onTest={handleTest} onCancel={() => setShowAddForm(false)} />
          </div>
        )}

        <div className="space-y-4">
          {servers.length === 0 ? (
            <div className="text-center py-12 bg-white rounded-lg border">
              <p className="text-gray-500">No MCP servers configured.</p>
              <p className="text-sm text-gray-400 mt-1">Add a server to get started.</p>
            </div>
          ) : (
            servers.map((server) => <ServerListItem key={server.id} server={server} onToggle={(enabled) => handleToggle(server.id, enabled)} onExpand={() => handleExpand(server.id)} onDelete={server.isOwned ? () => handleDelete(server.id) : undefined} isExpanded={expandedServerId === server.id} tools={serverTools[server.id] || []} />)
          )}
        </div>
      </div>
    </div>
  );
}
