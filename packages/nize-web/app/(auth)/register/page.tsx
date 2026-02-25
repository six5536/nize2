"use client";

// @awa-component: CFG-RegisterPage

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth-context";

export default function RegisterPage() {
  const router = useRouter();
  const { register } = useAuth();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");

    if (password.length < 8) {
      setError("Password must be at least 8 characters");
      return;
    }

    if (password !== confirmPassword) {
      setError("Passwords do not match");
      return;
    }

    setLoading(true);

    try {
      const result = await register(email, password, name);

      if (!result.success) {
        setError(result.error || "Registration failed");
        return;
      }

      // Registration successful, user is logged in
      router.push("/");
    } catch {
      setError("Something went wrong. Please try again.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <div style={styles.header}>
          <h1 style={styles.title}>Create Account</h1>
          <p style={styles.subtitle}>Join Nize to get started</p>
        </div>

        <form onSubmit={handleSubmit}>
          {error && <div style={styles.error}>{error}</div>}

          <div style={styles.field}>
            <label htmlFor="name" style={styles.label}>
              Name
            </label>
            <input id="name" type="text" value={name} onChange={(e) => setName(e.target.value)} required style={styles.input} placeholder="Your name" />
          </div>

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
            <input id="password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} required minLength={8} style={styles.input} placeholder="••••••••" />
            <p style={styles.hint}>Minimum 8 characters</p>
          </div>

          <div style={styles.field}>
            <label htmlFor="confirmPassword" style={styles.label}>
              Confirm Password
            </label>
            <input id="confirmPassword" type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)} required style={styles.input} placeholder="••••••••" />
          </div>

          <button
            type="submit"
            disabled={loading}
            style={{
              ...styles.button,
              ...(loading ? styles.buttonDisabled : {}),
            }}
          >
            {loading ? "Creating account..." : "Create account"}
          </button>
        </form>

        <div style={styles.footer}>
          <span>Already have an account? </span>
          <Link href="/login" style={styles.link}>
            Sign in
          </Link>
        </div>
      </div>
    </div>
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
  hint: {
    marginTop: "0.25rem",
    fontSize: "0.75rem",
    color: "#999",
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
