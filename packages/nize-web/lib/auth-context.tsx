"use client";

// @awa-component: CFG-NizeWebAuthContext

/**
 * Auth context for cookie-based authentication.
 * Uses httpOnly cookies for tokens (set by API), localStorage for user info only.
 */

import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from "react";
import { apiUrl } from "./api";

// User info storage key (not sensitive — tokens are in httpOnly cookies)
const USER_KEY = "nize_user";

export interface User {
  id: string;
  email: string;
  name?: string;
  roles?: string[];
}

interface AuthContextType {
  user: User | null;
  isLoading: boolean;
  isAuthenticated: boolean;
  login: (email: string, password: string) => Promise<{ success: boolean; error?: string }>;
  register: (email: string, password: string, name?: string) => Promise<{ success: boolean; error?: string }>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | null>(null);

// Helper to safely access localStorage (only on client)
function getStoredUser(): User | null {
  if (typeof window === "undefined") return null;
  const stored = localStorage.getItem(USER_KEY);
  if (!stored) return null;
  try {
    return JSON.parse(stored);
  } catch {
    return null;
  }
}

function setStoredUser(user: User): void {
  if (typeof window === "undefined") return;
  localStorage.setItem(USER_KEY, JSON.stringify(user));
}

function clearStoredUser(): void {
  if (typeof window === "undefined") return;
  localStorage.removeItem(USER_KEY);
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Validate session by trying to refresh tokens (cookies sent automatically)
  const validateSession = useCallback(async (): Promise<User | null> => {
    try {
      const res = await fetch(apiUrl("/auth/refresh"), {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}), // Empty body — refresh token comes from cookie
      });

      if (!res.ok) {
        clearStoredUser();
        setUser(null);
        return null;
      }

      const data = await res.json();
      if (data.user) {
        setStoredUser(data.user);
        setUser(data.user);
        return data.user;
      }
      return null;
    } catch {
      // Network error — keep user state but may fail on next API call
      return null;
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Initialize from localStorage on mount, then validate with API
  useEffect(() => {
    const storedUser = getStoredUser();
    if (storedUser) {
      setUser(storedUser);
    }
    // Validate session regardless (handles new browser sessions with valid cookies)
    validateSession();
  }, [validateSession]);

  const login = useCallback(async (email: string, password: string): Promise<{ success: boolean; error?: string }> => {
    try {
      const res = await fetch(apiUrl("/auth/login"), {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email, password }),
      });

      const data = await res.json();

      if (!res.ok) {
        return { success: false, error: data.message || data.error || "Login failed" };
      }

      // Store user info (tokens are in httpOnly cookies)
      setStoredUser(data.user);
      setUser(data.user);

      return { success: true };
    } catch {
      return { success: false, error: "Network error" };
    }
  }, []);

  const register = useCallback(async (email: string, password: string, name?: string): Promise<{ success: boolean; error?: string }> => {
    try {
      const res = await fetch(apiUrl("/auth/register"), {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email, password, name }),
      });

      const data = await res.json();

      if (!res.ok) {
        return { success: false, error: data.message || data.error || "Registration failed" };
      }

      // Store user info (tokens are in httpOnly cookies)
      setStoredUser(data.user);
      setUser(data.user);

      return { success: true };
    } catch {
      return { success: false, error: "Network error" };
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      await fetch(apiUrl("/auth/logout"), {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
    } catch {
      // Ignore logout errors
    }

    clearStoredUser();
    setUser(null);
  }, []);

  return (
    <AuthContext.Provider
      value={{
        user,
        isLoading,
        isAuthenticated: !!user,
        login,
        register,
        logout,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}

/**
 * Hook for making authenticated API calls.
 * Cookies are sent automatically with credentials: "include".
 */
export function useAuthFetch() {
  const { logout } = useAuth();

  return useCallback(
    async (path: string, options: RequestInit = {}): Promise<Response> => {
      const res = await fetch(apiUrl(path), {
        ...options,
        credentials: "include",
      });

      // If unauthorized, clear auth state
      if (res.status === 401) {
        await logout();
      }

      return res;
    },
    [logout],
  );
}
