// @zen-component: PLAN-032-UseOAuthFlow

/**
 * Hook consolidating OAuth popup/poll logic.
 *
 * Used by both Test Connection (auto-auth) and Re-authorize flows.
 * Handles browser popups with postMessage and Tauri system browser with polling.
 */

"use client";

import { useCallback, useRef, useState } from "react";
import { openExternal } from "@/lib/tauri";

interface OAuthFlowResult {
  success: boolean;
  error?: string;
}

interface UseOAuthFlowReturn {
  /** Whether an OAuth flow is currently in progress. */
  inProgress: boolean;
  /**
   * Start an OAuth flow: open popup/browser, wait for completion via
   * postMessage or status polling.
   *
   * @param authUrl URL to open for OAuth consent
   * @param serverId Server ID to match postMessage events and poll status
   * @param authFetch Authenticated fetch for polling /oauth/status
   * @param timeoutMs Timeout in ms (default 120000 = 2 min)
   */
  startOAuthFlow: (authUrl: string, serverId: string, authFetch: (path: string, options?: RequestInit) => Promise<Response>, timeoutMs?: number) => Promise<OAuthFlowResult>;
  /** Cancel any in-progress flow. */
  cancel: () => void;
}

// @zen-impl: PLAN-032 Step 3
export function useOAuthFlow(): UseOAuthFlowReturn {
  const [inProgress, setInProgress] = useState(false);
  const cancelRef = useRef<(() => void) | null>(null);

  const cancel = useCallback(() => {
    cancelRef.current?.();
    cancelRef.current = null;
  }, []);

  const startOAuthFlow = useCallback(async (authUrl: string, serverId: string, authFetch: (path: string, options?: RequestInit) => Promise<Response>, timeoutMs = 120000): Promise<OAuthFlowResult> => {
    setInProgress(true);
    try {
      const popup = await openExternal(authUrl, "oauth-connect", "width=600,height=700");

      if (popup) {
        // Browser: listen for postMessage from OAuth callback
        return await new Promise<OAuthFlowResult>((resolve) => {
          const cleanup = () => {
            window.removeEventListener("message", listener);
            clearInterval(pollClosed);
            clearTimeout(timeout);
            cancelRef.current = null;
          };

          cancelRef.current = () => {
            cleanup();
            if (popup && !popup.closed) popup.close();
            resolve({ success: false, error: "OAuth flow cancelled" });
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

          const timeout = setTimeout(() => {
            cleanup();
            if (popup && !popup.closed) popup.close();
            resolve({ success: false, error: "OAuth authorization timed out" });
          }, timeoutMs);

          const pollClosed = setInterval(() => {
            if (popup && popup.closed) {
              cleanup();
              resolve({ success: false, error: "OAuth window was closed" });
            }
          }, 500);
        });
      } else {
        // Tauri / no popup: poll OAuth status endpoint until connected
        return await new Promise<OAuthFlowResult>((resolve) => {
          let stopped = false;

          const cleanup = () => {
            stopped = true;
            clearTimeout(timeout);
            clearInterval(poll);
            cancelRef.current = null;
          };

          cancelRef.current = () => {
            cleanup();
            resolve({ success: false, error: "OAuth flow cancelled" });
          };

          const timeout = setTimeout(() => {
            cleanup();
            resolve({ success: false, error: "OAuth authorization timed out" });
          }, timeoutMs);

          const poll = setInterval(async () => {
            if (stopped) return;
            try {
              const res = await authFetch(`/mcp/servers/${serverId}/oauth/status`);
              if (res.ok) {
                const status = await res.json();
                if (status.connected) {
                  cleanup();
                  resolve({ success: true });
                }
              }
            } catch {
              /* ignore poll errors */
            }
          }, 2000);
        });
      }
    } finally {
      setInProgress(false);
    }
  }, []);

  return { inProgress, startOAuthFlow, cancel };
}
