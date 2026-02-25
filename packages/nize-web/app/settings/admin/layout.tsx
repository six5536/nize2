// @awa-component: PLAN-020-AdminSettingsGuard

/**
 * Admin settings guard layout.
 * Redirects non-admin users to /settings.
 */

"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth-context";

export default function AdminSettingsLayout({ children }: { children: React.ReactNode }) {
  const { isLoading, user } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (isLoading) return;
    if (!user?.roles?.includes("admin")) {
      router.replace("/settings");
    }
  }, [isLoading, user, router]);

  if (isLoading || !user?.roles?.includes("admin")) {
    return null;
  }

  return <>{children}</>;
}
