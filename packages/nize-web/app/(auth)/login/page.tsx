"use client";

// @zen-component: CFG-LoginPage

import { useState, Suspense } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { useAuth } from "@/lib/auth-context";

function LoginForm() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const { login } = useAuth();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);

    try {
      const result = await login(email, password);
      if (result.success) {
        const callbackUrl = searchParams.get("callbackUrl") || "/";
        router.push(callbackUrl);
      } else {
        setError(result.error || "Invalid email or password");
      }
    } catch (err: any) {
      setError(err?.message || "Invalid email or password");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <div style={styles.header}>
          <h1 style={styles.title}>Welcome to Nize</h1>
          <p style={styles.subtitle}>Sign in to access your workspace</p>
        </div>

        <form onSubmit={handleSubmit}>
          {error && <div style={styles.error}>{error}</div>}

          <div style={styles.field}>
            <label htmlFor="email" style={styles.label}>
              Email
            </label>
            <input id="email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required style={styles.input} placeholder="you@example.com" />
          </div>

          <div style={styles.field}>
            <label htmlFor="password" style={styles.label}>
              Password
            </label>
            <input id="password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} required style={styles.input} placeholder="•••••••••" />
          </div>

          <button
            type="submit"
            disabled={loading}
            style={{
              ...styles.button,
              ...(loading ? styles.buttonDisabled : {}),
            }}
          >
            {loading ? "Signing in..." : "Sign in"}
          </button>
        </form>

        <div style={styles.footer}>
          <span>Don&apos;t have an account? </span>
          <a href="/register" style={styles.link}>
            Sign up
          </a>
        </div>
      </div>
    </div>
  );
}

export default function LoginPage() {
  return (
    <Suspense
      fallback={
        <div style={styles.container}>
          <div style={styles.card}>
            <p style={{ textAlign: "center", color: "#666" }}>Loading...</p>
          </div>
        </div>
      }
    >
      <LoginForm />
    </Suspense>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    minHeight: "100vh",
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: "#f5f5f5",
    padding: "1rem",
    fontFamily: "system-ui, sans-serif",
  },
  card: {
    width: "100%",
    maxWidth: "400px",
    backgroundColor: "#fff",
    borderRadius: "8px",
    boxShadow: "0 2px 8px rgba(0,0,0,0.1)",
    padding: "2rem",
  },
  header: {
    marginBottom: "1.5rem",
    textAlign: "center" as const,
  },
  title: {
    fontSize: "1.5rem",
    fontWeight: 700,
    color: "#111",
    margin: 0,
  },
  subtitle: {
    fontSize: "0.875rem",
    color: "#666",
    marginTop: "0.25rem",
  },
  error: {
    backgroundColor: "#fef2f2",
    color: "#991b1b",
    padding: "0.75rem",
    borderRadius: "6px",
    fontSize: "0.875rem",
    marginBottom: "1rem",
  },
  field: {
    marginBottom: "1rem",
  },
  label: {
    display: "block",
    fontSize: "0.875rem",
    fontWeight: 500,
    color: "#333",
    marginBottom: "0.25rem",
  },
  input: {
    width: "100%",
    padding: "0.5rem 0.75rem",
    border: "1px solid #d1d5db",
    borderRadius: "6px",
    fontSize: "0.875rem",
    outline: "none",
    boxSizing: "border-box" as const,
  },
  button: {
    width: "100%",
    padding: "0.625rem 1rem",
    backgroundColor: "#2563eb",
    color: "#fff",
    border: "none",
    borderRadius: "6px",
    fontSize: "0.875rem",
    fontWeight: 500,
    cursor: "pointer",
    marginTop: "0.5rem",
  },
  buttonDisabled: {
    opacity: 0.5,
    cursor: "not-allowed",
  },
  footer: {
    marginTop: "1.5rem",
    textAlign: "center" as const,
    fontSize: "0.875rem",
    color: "#666",
  },
  link: {
    color: "#2563eb",
    textDecoration: "none",
    fontWeight: 500,
  },
};
