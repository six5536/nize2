import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AuthProvider } from "./auth";
import { AuthGate } from "./AuthGate";

function App() {
  const [apiPort, setApiPort] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function getPort() {
      try {
        const port = await invoke<number>("get_api_port");
        console.log("[app] got api port:", port);
        if (!cancelled) setApiPort(port);
      } catch (e) {
        console.error("[app] get_api_port failed:", e);
        if (!cancelled) setError(String(e));
      }
    }

    getPort();
    return () => {
      cancelled = true;
    };
  }, []);

  if (error) {
    return (
      <main style={{ fontFamily: "system-ui, sans-serif", padding: "2rem" }}>
        <h1>Nize Desktop</h1>
        <p style={{ color: "red" }}>Failed to connect to API: {error}</p>
      </main>
    );
  }

  if (apiPort === null) {
    return (
      <main style={{ fontFamily: "system-ui, sans-serif", padding: "2rem" }}>
        <p>Starting…</p>
      </main>
    );
  }

  return (
    <AuthProvider apiPort={apiPort}>
      <AuthGate />
    </AuthProvider>
  );
}

export default App;
