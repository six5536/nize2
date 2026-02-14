// @zen-component: CFG-NizeWebApi

/**
 * API configuration for connecting to the nize API server.
 *
 * In sidecar mode the nize-web-server.mjs wrapper writes a
 * `public/__nize-env.js` file that sets `window.__NIZE_ENV__`
 * before starting the Next.js server. This keeps all pages
 * fully static (no SSR/dynamic rendering needed).
 *
 * For cloud deployment, the deploy script writes the same file
 * or sets NEXT_PUBLIC_API_URL at build time.
 */

declare global {
  interface Window {
    __NIZE_ENV__?: { apiPort?: string };
  }
}

function getApiBaseUrl(): string {
  // Client-side: when running behind nize-web-server proxy, use relative
  // URLs so API requests go through the same origin (first-party cookies).
  if (typeof window !== "undefined" && window.__NIZE_ENV__?.apiPort) {
    return "";
  }
  // Build-time fallback (cloud deployment)
  if (typeof process !== "undefined" && process.env?.NEXT_PUBLIC_API_URL) {
    return process.env.NEXT_PUBLIC_API_URL;
  }
  return "http://127.0.0.1:3001";
}

/**
 * Build a full API URL from a path.
 */
export function apiUrl(path: string): string {
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  return `${getApiBaseUrl()}${normalizedPath}`;
}
