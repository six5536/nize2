import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "./auth";
import { UpdateChecker } from "./UpdateChecker";
import { McpClientSettings } from "./settings/McpClientSettings";

interface HelloResponse {
  greeting: string;
  dbConnected: boolean;
  nodeAvailable: boolean;
  nodeVersion: string | null;
}

/**
 * Main application content — shown after authentication.
 */
export function MainApp() {
  const { user, logout } = useAuth();
  const [response, setResponse] = useState<HelloResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleHelloClick() {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<HelloResponse>("hello_world");
      setResponse(result);
    } catch (e) {
      setError(String(e));
      setResponse(null);
    } finally {
      setLoading(false);
    }
  }

  return (
    <main style={{ fontFamily: "system-ui, sans-serif", padding: "2rem" }}>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h1>Nize Desktop</h1>
        <div>
          <span style={{ marginRight: "1rem", color: "#666" }}>
            {user?.email}
            {user?.roles.includes("admin") && " (admin)"}
          </span>
          <button onClick={logout}>Sign Out</button>
        </div>
      </header>

      <UpdateChecker />

      <section style={{ marginTop: "2rem" }}>
        <button onClick={handleHelloClick} disabled={loading}>
          {loading ? "Checking…" : "Hello World"}
        </button>

        {error && <p style={{ color: "red", marginTop: "1rem" }}>Error: {error}</p>}

        {response && (
          <div style={{ marginTop: "1rem" }}>
            <p>
              <strong>Greeting:</strong> {response.greeting}
            </p>
            <p>
              <strong>Database:</strong> <span style={{ color: response.dbConnected ? "green" : "red" }}>{response.dbConnected ? "✓ Connected" : "✗ Unavailable"}</span>
            </p>
            <p>
              <strong>Node.js:</strong> <span style={{ color: response.nodeAvailable ? "green" : "red" }}>{response.nodeAvailable ? `✓ ${response.nodeVersion}` : "✗ Unavailable"}</span>
            </p>
          </div>
        )}
      </section>

      <section style={{ marginTop: "2rem", borderTop: "1px solid #ddd", paddingTop: "1.5rem" }}>
        <McpClientSettings />
      </section>
    </main>
  );
}
