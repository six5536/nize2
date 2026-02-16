// @zen-impl: PLAN-021 — desktop settings page (Tauri-only)
// Ported from packages/nize-desktop/src/MainApp.tsx

"use client";

import { useState, useEffect } from "react";
import { isTauri } from "@/lib/tauri";

export default function DesktopSettingsPage() {
  const [isDesktop, setIsDesktop] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    setIsDesktop(true);
  }, []);

  if (!isDesktop) {
    return (
      <div className="py-12 text-center text-gray-500">
        <p>Desktop settings are only available in the Nize desktop app.</p>
      </div>
    );
  }

  return <DesktopSettingsContent />;
}

/**
 * Lazy-loaded desktop settings content.
 * Separate component so Tauri imports only load when needed.
 */
function DesktopSettingsContent() {
  const [McpClientSettings, setMcpClientSettings] = useState<React.ComponentType | null>(null);
  const [UpdateChecker, setUpdateChecker] = useState<React.ComponentType | null>(null);
  const [HelloResponse, setHelloResponse] = useState<{ greeting: string; dbConnected: boolean; bunAvailable: boolean; bunVersion: string | null } | null>(null);
  const [helloError, setHelloError] = useState<string | null>(null);
  const [helloLoading, setHelloLoading] = useState(false);

  useEffect(() => {
    // Dynamically import desktop-only components
    import("@/components/desktop/McpClientSettings").then((mod) => setMcpClientSettings(() => mod.McpClientSettings));
    import("@/components/desktop/UpdateChecker").then((mod) => setUpdateChecker(() => mod.UpdateChecker));
  }, []);

  async function handleHelloClick() {
    setHelloLoading(true);
    setHelloError(null);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const result = await invoke<{ greeting: string; dbConnected: boolean; bunAvailable: boolean; bunVersion: string | null }>("hello_world");
      setHelloResponse(result);
    } catch (e) {
      setHelloError(String(e));
      setHelloResponse(null);
    } finally {
      setHelloLoading(false);
    }
  }

  return (
    <div>
      {UpdateChecker && <UpdateChecker />}

      <section style={{ marginTop: "1rem" }}>
        <button onClick={handleHelloClick} disabled={helloLoading}>
          {helloLoading ? "Checking…" : "Hello World"}
        </button>

        {helloError && <p style={{ color: "red", marginTop: "1rem" }}>Error: {helloError}</p>}

        {HelloResponse && (
          <div style={{ marginTop: "1rem" }}>
            <p>
              <strong>Greeting:</strong> {HelloResponse.greeting}
            </p>
            <p>
              <strong>Database:</strong> <span style={{ color: HelloResponse.dbConnected ? "green" : "red" }}>{HelloResponse.dbConnected ? "✓ Connected" : "✗ Unavailable"}</span>
            </p>
            <p>
              <strong>Bun:</strong> <span style={{ color: HelloResponse.bunAvailable ? "green" : "red" }}>{HelloResponse.bunAvailable ? `✓ ${HelloResponse.bunVersion}` : "✗ Unavailable"}</span>
            </p>
          </div>
        )}
      </section>

      <section style={{ marginTop: "2rem", borderTop: "1px solid #ddd", paddingTop: "1.5rem" }}>{McpClientSettings && <McpClientSettings />}</section>
    </div>
  );
}
