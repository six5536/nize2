// @zen-impl: PLAN-021 — webview bridge loader for Tauri dev builds
"use client";

import { useEffect } from "react";
import { isTauri } from "@/lib/tauri";

/**
 * Initializes the webview bridge in dev mode when running inside Tauri.
 * Renders nothing — just a side-effect component.
 */
export function WebviewBridgeLoader() {
  useEffect(() => {
    if (process.env.NODE_ENV === "development" && isTauri()) {
      import("@/lib/webview-bridge").then(({ initWebviewBridge }) => {
        initWebviewBridge();
      });
    }
  }, []);

  return null;
}
