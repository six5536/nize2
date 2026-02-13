import { useState, type FormEvent } from "react";
import { useAuth } from "./AuthContext";

export function RegisterPage() {
  const { register, error, clearError } = useAuth();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setLoading(true);
    try {
      await register(email, password, name || undefined);
    } catch {
      // error is set in context
    } finally {
      setLoading(false);
    }
  }

  return (
    <main style={{ fontFamily: "system-ui, sans-serif", padding: "2rem", maxWidth: "400px", margin: "0 auto" }}>
      <h1>Create Admin Account</h1>
      <p style={{ color: "#666", marginBottom: "1.5rem" }}>Welcome to Nize! Create the first admin account to get started.</p>
      <form onSubmit={handleSubmit}>
        <div style={{ marginBottom: "1rem" }}>
          <label htmlFor="name" style={{ display: "block", marginBottom: "0.25rem" }}>
            Name
          </label>
          <input id="name" type="text" value={name} onChange={(e) => setName(e.target.value)} placeholder="Optional" style={{ width: "100%", padding: "0.5rem", boxSizing: "border-box" }} />
        </div>
        <div style={{ marginBottom: "1rem" }}>
          <label htmlFor="email" style={{ display: "block", marginBottom: "0.25rem" }}>
            Email
          </label>
          <input
            id="email"
            type="email"
            value={email}
            onChange={(e) => {
              clearError();
              setEmail(e.target.value);
            }}
            required
            style={{ width: "100%", padding: "0.5rem", boxSizing: "border-box" }}
          />
        </div>
        <div style={{ marginBottom: "1rem" }}>
          <label htmlFor="password" style={{ display: "block", marginBottom: "0.25rem" }}>
            Password
          </label>
          <input
            id="password"
            type="password"
            value={password}
            onChange={(e) => {
              clearError();
              setPassword(e.target.value);
            }}
            required
            minLength={8}
            placeholder="Minimum 8 characters"
            style={{ width: "100%", padding: "0.5rem", boxSizing: "border-box" }}
          />
        </div>
        {error && <p style={{ color: "red" }}>{error}</p>}
        <button type="submit" disabled={loading} style={{ padding: "0.5rem 1.5rem" }}>
          {loading ? "Creating account…" : "Create Account"}
        </button>
      </form>
    </main>
  );
}
