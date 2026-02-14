// @zen-impl: PLAN-012-1.2 — hello world page
// @zen-impl: CFG-NizeWebAuthContext — auth-aware home page

"use client";

import Link from "next/link";
import { useAuth } from "@/lib/auth-context";
import { AuthGate } from "@/lib/auth-gate";

function HomePage() {
  const { user, logout } = useAuth();

  return (
    <main
      style={{
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        alignItems: "center",
        minHeight: "100vh",
        fontFamily: "system-ui, sans-serif",
        gap: "1rem",
      }}
    >
      <h1>Hello from nize-web</h1>
      {user && <p style={{ color: "#666" }}>Signed in as {user.email}</p>}
      <nav style={{ display: "flex", gap: "1rem" }}>
        <Link href="/settings" style={{ color: "#2563eb", textDecoration: "none" }}>
          Settings
        </Link>
        <button
          onClick={() => logout()}
          style={{
            background: "none",
            border: "none",
            color: "#2563eb",
            cursor: "pointer",
            fontSize: "inherit",
            fontFamily: "inherit",
          }}
        >
          Sign out
        </button>
      </nav>
    </main>
  );
}

export default function Home() {
  return (
    <AuthGate>
      <HomePage />
    </AuthGate>
  );
}
