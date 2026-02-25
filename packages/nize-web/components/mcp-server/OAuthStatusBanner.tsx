// @awa-component: PLAN-032-OAuthStatusBanner

/**
 * OAuth connection status banner with Re-authorize button.
 *
 * Re-authorize: revoke → initiate → OAuth flow → refresh status.
 * No separate Disconnect button — Re-authorize handles the full cycle.
 */

"use client";

import { useEffect, useState, useCallback } from "react";
import type { OAuthStatus } from "./types";
import { useOAuthFlow } from "./useOAuthFlow";

interface OAuthStatusBannerProps {
  serverId: string;
  authFetch: (path: string, options?: RequestInit) => Promise<Response>;
  /** Called after a successful re-authorize to let parent refresh state. */
  onStatusChange?: (status: OAuthStatus) => void;
  /** Called when an error occurs. */
  onError?: (error: string) => void;
}

// @awa-impl: PLAN-032 Step 5
export function OAuthStatusBanner({ serverId, authFetch, onStatusChange, onError }: OAuthStatusBannerProps) {
  const [oauthStatus, setOauthStatus] = useState<OAuthStatus | null>(null);
  const [revoking, setRevoking] = useState(false);
  const { inProgress, startOAuthFlow } = useOAuthFlow();

  const fetchStatus = useCallback(async () => {
    try {
      const res = await authFetch(`/mcp/servers/${serverId}/oauth/status`);
      if (res.ok) {
        const data = await res.json();
        setOauthStatus(data);
        onStatusChange?.(data);
      }
    } catch {
      // ignore — status is optional
    }
  }, [serverId, authFetch, onStatusChange]);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  // @awa-impl: PLAN-032 Step 8 — Re-authorize: revoke → initiate → flow → refresh
  const handleReauthorize = async () => {
    onError?.(undefined as unknown as string); // clear previous error
    try {
      // 1. Revoke existing token
      await authFetch(`/mcp/servers/${serverId}/oauth/revoke`, { method: "POST" });

      // 2. Initiate fresh OAuth flow
      const oauthRes = await authFetch(`/mcp/servers/${serverId}/oauth/initiate`, { method: "POST" });
      if (!oauthRes.ok) {
        const data = await oauthRes.json().catch(() => ({}));
        throw new Error(data.message || "Failed to initiate OAuth");
      }
      const { authUrl } = await oauthRes.json();

      // 3. OAuth popup / poll
      const result = await startOAuthFlow(authUrl, serverId, authFetch);

      if (result.success) {
        await fetchStatus();
      } else {
        onError?.(result.error || "OAuth failed");
      }
    } catch (err) {
      onError?.(err instanceof Error ? err.message : "OAuth re-authorization failed");
    }
  };

  const handleRevoke = async () => {
    onError?.(undefined as unknown as string); // clear previous error
    setRevoking(true);
    try {
      const revokeRes = await authFetch(`/mcp/servers/${serverId}/oauth/revoke`, { method: "POST" });
      if (!revokeRes.ok) {
        const data = await revokeRes.json().catch(() => ({}));
        throw new Error(data.message || "Failed to revoke OAuth token");
      }
      await fetchStatus();
    } catch (err) {
      onError?.(err instanceof Error ? err.message : "OAuth revoke failed");
    } finally {
      setRevoking(false);
    }
  };

  if (!oauthStatus) return null;

  const expiresAtDate = oauthStatus.expiresAt ? new Date(oauthStatus.expiresAt) : null;
  const isExpired = expiresAtDate ? expiresAtDate.getTime() <= Date.now() : false;

  return (
    <div className={`p-3 rounded-md ${oauthStatus.connected ? "bg-green-50 border border-green-200" : "bg-yellow-50 border border-yellow-200"}`}>
      <div className="flex items-center justify-between">
        <div>
          <p className={`text-sm font-medium ${oauthStatus.connected ? "text-green-800" : "text-yellow-800"}`}>{oauthStatus.connected ? "\u2713 OAuth Connected" : "\u26A0 OAuth Not Connected"}</p>
          {expiresAtDate && (
            <p className="text-xs text-gray-500 mt-1">
              Token expiry: {expiresAtDate.toLocaleString()} {isExpired ? "(expired)" : "(active)"}
            </p>
          )}
        </div>
        <div className="flex gap-2">
          <button type="button" onClick={handleRevoke} disabled={inProgress || revoking} className="px-3 py-1 text-xs font-medium text-red-700 bg-red-100 rounded-md hover:bg-red-200 disabled:opacity-50">
            {revoking ? "Revoking..." : "Revoke"}
          </button>
          <button type="button" onClick={handleReauthorize} disabled={inProgress} className="px-3 py-1 text-xs font-medium text-blue-700 bg-blue-100 rounded-md hover:bg-blue-200 disabled:opacity-50">
            {inProgress ? "Authorizing..." : "Re-authorize"}
          </button>
        </div>
      </div>
    </div>
  );
}
