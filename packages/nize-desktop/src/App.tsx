import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface HelloResponse {
  greeting: string;
  dbConnected: boolean;
  nodeAvailable: boolean;
  nodeVersion: string | null;
}

function App() {
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
      <h1>Nize Desktop</h1>
      <p>Tauri + React bootstrap — ready for development.</p>

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
    </main>
  );
}

export default App;
