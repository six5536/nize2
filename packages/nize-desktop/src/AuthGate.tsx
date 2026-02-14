// @zen-impl: PLAN-012-5.2
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAuth, LoginPage, RegisterPage } from "./auth";
import { MainApp } from "./MainApp";
import { TabView } from "./TabView";

/**
 * Auth gate — routes to the correct screen based on auth state:
 * - Loading spinner while checking
 * - RegisterPage if no admin exists (first run)
 * - LoginPage if admin exists but not authenticated
 * - TabView (Desktop + Web tabs) if authenticated
 */
export function AuthGate() {
  const { user, adminExists, loading } = useAuth();
  const [nizeWebPort, setNizeWebPort] = useState<number | null>(null);

  useEffect(() => {
    if (user) {
      invoke<number>("get_nize_web_port")
        .then(setNizeWebPort)
        .catch(() => {}); // Sidecar may not be available yet
    }
  }, [user]);

  if (loading) {
    return (
      <main style={{ fontFamily: "system-ui, sans-serif", padding: "2rem" }}>
        <p>Checking authentication…</p>
      </main>
    );
  }

  // Authenticated — show tabbed view with Desktop and Web tabs
  if (user) {
    const tabs = [
      { id: "desktop", label: "Desktop", content: <MainApp /> },
      {
        id: "web",
        label: "Web",
        content: nizeWebPort ? (
          <iframe src={`http://localhost:${nizeWebPort}`} style={{ width: "100%", height: "100%", border: "none" }} title="nize-web" />
        ) : (
          <main style={{ fontFamily: "system-ui, sans-serif", padding: "2rem" }}>
            <p>Loading nize-web…</p>
          </main>
        ),
        disabled: !nizeWebPort,
      },
    ];

    return <TabView tabs={tabs} />;
  }

  // First run — no admin exists
  if (adminExists === false) {
    return <RegisterPage />;
  }

  // Admin exists (or status check failed) — show login
  return <LoginPage />;
}
