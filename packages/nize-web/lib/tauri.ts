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
