// @zen-component: CFG-NizeWebApi
// @zen-impl: PLAN-021 — Tauri IPC port discovery with fallbacks

/**
 * API configuration for connecting to the nize API server.
 *
 * Resolution order:
 * 1. Tauri IPC — `invoke("get_api_port")` when running in the Tauri webview
 * 2. `window.__NIZE_ENV__` — injected by nize-web-server.mjs (production sidecar)
 * 3. `NEXT_PUBLIC_API_URL` — build-time env var (cloud deployment)
 * 4. Relative URLs — when proxied by Next.js rewrites (dev mode)
 */

import { isTauri } from "./tauri";

declare global {
  interface Window {
    __NIZE_ENV__?: { apiPort?: string };
  }
}

// Cached API base URL resolved via Tauri IPC (set once, used forever).
let tauriApiBaseUrl: string | null = null;
let tauriPortPromise: Promise<string> | null = null;

/**
 * Resolve the API port via Tauri IPC and cache it.
 * Returns the base URL (e.g. "http://127.0.0.1:3001").
 */
async function resolveTauriApiBaseUrl(): Promise<string> {
  if (tauriApiBaseUrl) return tauriApiBaseUrl;
  if (!tauriPortPromise) {
    tauriPortPromise = (async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const port = await invoke<number>("get_api_port");
      tauriApiBaseUrl = `http://127.0.0.1:${port}`;
      return tauriApiBaseUrl;
    })();
  }
  return tauriPortPromise;
}

function getApiBaseUrl(): string {
  // Tauri: use cached base URL if already resolved
  if (tauriApiBaseUrl) return tauriApiBaseUrl;

  // Sidecar production: nize-web-server.mjs proxies API routes — use relative URLs
  if (typeof window !== "undefined" && window.__NIZE_ENV__?.apiPort) {
    return "";
  }
  // Build-time fallback (cloud deployment)
  if (typeof process !== "undefined" && process.env?.NEXT_PUBLIC_API_URL) {
    return process.env.NEXT_PUBLIC_API_URL;
  }
  // Dev mode: Next.js rewrites proxy to API sidecar — use relative URLs
  return "";
}

/**
 * Build a full API URL from a path.
 *
 * All API routes are mounted under `/api` on the server, so this
 * function prepends `/api` to the given path automatically.
 *
 * When running in Tauri and the port hasn't been resolved yet, this
 * returns a relative URL (which works via Next.js rewrites in dev).
 * Use `apiUrlAsync()` when you need the absolute URL.
 */
export function apiUrl(path: string): string {
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  return `${getApiBaseUrl()}/api${normalizedPath}`;
}

/**
 * Async version of `apiUrl()` that resolves the Tauri API port via IPC
 * before building the URL.  Falls back to `apiUrl()` outside Tauri.
 */
export async function apiUrlAsync(path: string): Promise<string> {
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  if (isTauri()) {
    const base = await resolveTauriApiBaseUrl();
    return `${base}/api${normalizedPath}`;
  }
  return apiUrl(path);
}
