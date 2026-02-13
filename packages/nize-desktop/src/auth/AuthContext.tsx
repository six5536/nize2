/**
 * Authentication context for the Nize desktop app.
 *
 * Manages access token (memory), refresh token (localStorage),
 * and auto-restore on app startup.
 */

import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from "react";
import { NizeApiClient, type TokenResponse, type AuthStatusResponse } from "@six5536/nize-api-client";

// ============================================================================
// Types
// ============================================================================

interface AuthUser {
  id: string;
  email: string;
  name?: string | null;
  roles: string[];
}

interface AuthState {
  /** Current user or null if not authenticated. */
  user: AuthUser | null;
  /** Whether an admin user exists in the system. */
  adminExists: boolean | null;
  /** True while the initial auth check is running. */
  loading: boolean;
  /** Most recent auth error message. */
  error: string | null;
}

interface AuthActions {
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, password: string, name?: string) => Promise<void>;
  logout: () => Promise<void>;
  clearError: () => void;
}

type AuthContextValue = AuthState & AuthActions;

// ============================================================================
// Constants
// ============================================================================

const REFRESH_TOKEN_KEY = "nize_refresh_token";

// ============================================================================
// Context
// ============================================================================

const AuthContext = createContext<AuthContextValue | null>(null);

// ============================================================================
// Provider
// ============================================================================

interface AuthProviderProps {
  apiPort: number;
  children: ReactNode;
}

export function AuthProvider({ apiPort, children }: AuthProviderProps) {
  const [state, setState] = useState<AuthState>({
    user: null,
    adminExists: null,
    loading: true,
    error: null,
  });
  const [accessToken, setAccessToken] = useState<string | null>(null);

  // Create API client with token injection
  const client = new NizeApiClient({
    baseUrl: `http://127.0.0.1:${apiPort}`,
    getToken: () => accessToken,
  });

  // --- Helpers ---

  const storeTokens = useCallback((resp: TokenResponse) => {
    setAccessToken(resp.accessToken);
    localStorage.setItem(REFRESH_TOKEN_KEY, resp.refreshToken);
    setState((s) => ({
      ...s,
      user: {
        id: resp.user.id,
        email: resp.user.email,
        name: resp.user.name,
        roles: resp.user.roles,
      },
      error: null,
    }));
  }, []);

  const clearTokens = useCallback(() => {
    setAccessToken(null);
    localStorage.removeItem(REFRESH_TOKEN_KEY);
    setState((s) => ({ ...s, user: null }));
  }, []);

  // --- Startup: check admin status + try auto-login ---

  useEffect(() => {
    const abort = new AbortController();

    async function init() {
      try {
        // Check if admin exists
        console.log("[auth] init: calling authStatus on", client["config"].baseUrl);
        const status: AuthStatusResponse = await client.authStatus({ signal: abort.signal });
        console.log("[auth] init: authStatus response", status);

        setState((s) => ({
          ...s,
          adminExists: status.adminExists,
        }));

        // Try auto-login from persisted refresh token
        const storedRefresh = localStorage.getItem(REFRESH_TOKEN_KEY);
        if (storedRefresh) {
          try {
            const resp = await client.refresh({ refreshToken: storedRefresh }, { signal: abort.signal });
            storeTokens(resp);
          } catch {
            // Refresh failed — stale token
            localStorage.removeItem(REFRESH_TOKEN_KEY);
          }
        }
      } catch (e) {
        if (abort.signal.aborted) return; // StrictMode cleanup — ignore
        console.error("[auth] init: error", e);
        // API not reachable yet — ignore, user will see login
      } finally {
        if (!abort.signal.aborted) {
          setState((s) => ({ ...s, loading: false }));
        }
      }
    }

    init();
    return () => {
      abort.abort();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [apiPort]);

  // --- Actions ---

  const login = useCallback(
    async (email: string, password: string) => {
      setState((s) => ({ ...s, error: null }));
      try {
        const resp = await client.login({ email, password });
        storeTokens(resp);
        setState((s) => ({ ...s, adminExists: true }));
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : "Login failed";
        setState((s) => ({ ...s, error: msg }));
        throw e;
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [apiPort],
  );

  const register = useCallback(
    async (email: string, password: string, name?: string) => {
      setState((s) => ({ ...s, error: null }));
      try {
        const resp = await client.register({
          email,
          password,
          ...(name ? { name } : {}),
        });
        storeTokens(resp);
        setState((s) => ({ ...s, adminExists: true }));
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : "Registration failed";
        setState((s) => ({ ...s, error: msg }));
        throw e;
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [apiPort],
  );

  const logout = useCallback(async () => {
    const refreshToken = localStorage.getItem(REFRESH_TOKEN_KEY);
    try {
      await client.logout(refreshToken ? { refreshToken } : undefined);
    } catch {
      // Best-effort — clear local state regardless
    }
    clearTokens();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [apiPort]);

  const clearError = useCallback(() => {
    setState((s) => ({ ...s, error: null }));
  }, []);

  const value: AuthContextValue = {
    ...state,
    login,
    register,
    logout,
    clearError,
  };

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

// ============================================================================
// Hook
// ============================================================================

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return ctx;
}
