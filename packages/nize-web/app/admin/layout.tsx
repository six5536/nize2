// @zen-component: PLAN-020-AdminLayout

/**
 * Admin layout with sidebar navigation.
 * Uses client-side role checking since nize-web is a static SPA.
 */

"use client";

import { useEffect } from "react";
import { useRouter, usePathname } from "next/navigation";
import { useAuth } from "@/lib/auth-context";

const adminNavItems = [
  { href: "/admin/tools", label: "MCP Servers" },
  { href: "/admin/settings", label: "Settings" },
];

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  const { isLoading, isAuthenticated, user } = useAuth();
  const router = useRouter();
  const pathname = usePathname();

  useEffect(() => {
    if (isLoading) return;
    if (!isAuthenticated) {
      router.replace("/login");
      return;
    }
    if (!user?.roles?.includes("admin")) {
      router.replace("/");
    }
  }, [isLoading, isAuthenticated, user, router]);

  if (isLoading || !isAuthenticated || !user?.roles?.includes("admin")) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex">
      <aside className="w-56 bg-gray-900 text-gray-300 flex flex-col">
        <div className="px-4 py-5 border-b border-gray-700">
          <h2 className="text-lg font-semibold text-white">Admin</h2>
        </div>
        <nav className="flex-1 px-2 py-4 space-y-1">
          {adminNavItems.map((item) => {
            const isActive = pathname.startsWith(item.href);
            return (
              <a key={item.href} href={item.href} className={`block px-3 py-2 rounded-md text-sm font-medium transition-colors ${isActive ? "bg-gray-800 text-white" : "hover:bg-gray-800 hover:text-white"}`}>
                {item.label}
              </a>
            );
          })}
        </nav>
        <div className="px-4 py-3 border-t border-gray-700 text-xs text-gray-500">
          <a href="/chat" className="hover:text-gray-300">
            &larr; Back to Chat
          </a>
        </div>
      </aside>
      <main className="flex-1">{children}</main>
    </div>
  );
}
