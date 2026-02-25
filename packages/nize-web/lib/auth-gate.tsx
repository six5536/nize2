"use client";

// @awa-component: CFG-AuthGate

/**
 * Auth gate component for protecting routes.
 *
 * - Wrapping a page in <AuthGate> redirects unauthenticated visitors to /login.
 * - Wrapping a page in <AuthGate publicOnly> redirects authenticated visitors
 *   to the home page (useful for login/register pages).
 */

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth-context";

interface AuthGateProps {
  children: React.ReactNode;
  /** If true, only show content to unauthenticated users (login/register pages). */
  publicOnly?: boolean;
}

export function AuthGate({ children, publicOnly }: AuthGateProps) {
  const { isAuthenticated, isLoading } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (isLoading) return;

    if (publicOnly && isAuthenticated) {
      router.replace("/");
    }

    if (!publicOnly && !isAuthenticated) {
      router.replace("/login");
    }
  }, [isAuthenticated, isLoading, publicOnly, router]);

  if (isLoading) {
    return (
      <div
        style={{
          display: "flex",
          minHeight: "100vh",
          alignItems: "center",
          justifyContent: "center",
          fontFamily: "system-ui, sans-serif",
          color: "#666",
        }}
      >
        Loading...
      </div>
    );
  }

  // Hide content during redirect
  if (publicOnly && isAuthenticated) return null;
  if (!publicOnly && !isAuthenticated) return null;

  return <>{children}</>;
}
