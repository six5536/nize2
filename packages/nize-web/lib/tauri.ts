// @zen-impl: PLAN-021 — Tauri detection and IPC helpers for nize-web
//
// When nize-web runs inside the Tauri webview, `window.__TAURI_INTERNALS__`
// is injected automatically.  All desktop-specific code should be gated
// behind `isTauri()` so nize-web works as a standalone web app too.

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

/**
 * Returns true when running inside a Tauri webview.
 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Open a URL in the system browser (Tauri) or in a new window (browser).
 * Returns the popup window when running in a browser, null in Tauri.
 */
export async function openExternal(url: string, windowName?: string, windowFeatures?: string): Promise<Window | null> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("plugin:shell|open", { path: url });
    return null;
  }
  return window.open(url, windowName, windowFeatures);
}
