import { useAuth, LoginPage, RegisterPage } from "./auth";
import { MainApp } from "./MainApp";

/**
 * Auth gate — routes to the correct screen based on auth state:
 * - Loading spinner while checking
 * - RegisterPage if no admin exists (first run)
 * - LoginPage if admin exists but not authenticated
 * - MainApp if authenticated
 */
export function AuthGate() {
  const { user, adminExists, loading } = useAuth();

  if (loading) {
    return (
      <main style={{ fontFamily: "system-ui, sans-serif", padding: "2rem" }}>
        <p>Checking authentication…</p>
      </main>
    );
  }

  // Authenticated — show main app
  if (user) {
    return <MainApp />;
  }

  // First run — no admin exists
  if (adminExists === false) {
    return <RegisterPage />;
  }

  // Admin exists (or status check failed) — show login
  return <LoginPage />;
}
