// @zen-impl: PLAN-021 — settings layout with sidebar navigation

"use client";

import { useEffect, useState } from "react";
import { useRouter, usePathname } from "next/navigation";
import { useAuth } from "@/lib/auth-context";
import { isTauri } from "@/lib/tauri";

interface NavItem {
  href: string;
  label: string;
  exact?: boolean;
}

const userNavItems: NavItem[] = [
  { href: "/settings", label: "General", exact: true },
  { href: "/settings/tools", label: "MCP Tools" },
];

const desktopNavItem: NavItem = { href: "/settings/desktop", label: "Desktop" };

const adminNavItems: NavItem[] = [
  { href: "/settings/admin/tools", label: "MCP Servers" },
];

export default function SettingsLayout({ children }: { children: React.ReactNode }) {
  const { isLoading, isAuthenticated, user } = useAuth();
  const router = useRouter();
  const pathname = usePathname();
  const [showDesktop, setShowDesktop] = useState(false);

  const isAdmin = user?.roles?.includes("admin");

  useEffect(() => {
    if (isLoading) return;
    if (!isAuthenticated) {
      router.replace("/login");
    }
  }, [isLoading, isAuthenticated, router]);

  useEffect(() => {
    setShowDesktop(isTauri());
  }, []);

  if (isLoading || !isAuthenticated) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
      </div>
    );
  }

  const navItems = showDesktop ? [...userNavItems, desktopNavItem] : userNavItems;

  const isActive = (item: NavItem) => (item.exact ? pathname === item.href : pathname.startsWith(item.href));

  return (
    <div className="h-screen flex bg-gray-50 overflow-hidden">
      {/* Sidebar — fixed height, nav scrolls independently */}
      <aside className="w-56 bg-white border-r border-gray-200 flex flex-col h-full shrink-0">
        <div className="px-4 py-5 border-b border-gray-200">
          <h2 className="text-lg font-semibold text-gray-900">Settings</h2>
        </div>
        <nav className="flex-1 overflow-y-auto px-2 py-4 space-y-1">
          {navItems.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className={`block px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                isActive(item)
                  ? "bg-blue-50 text-blue-700"
                  : "text-gray-600 hover:bg-gray-100 hover:text-gray-900"
              }`}
            >
              {item.label}
            </a>
          ))}

          {/* Admin Section */}
          {isAdmin && (
            <>
              <div className="pt-4 pb-2">
                <div className="border-t border-gray-200" />
                <p className="mt-3 px-3 text-xs font-semibold text-gray-400 uppercase tracking-wider">
                  Administration
                </p>
              </div>
              {adminNavItems.map((item) => (
                <a
                  key={item.href}
                  href={item.href}
                  className={`block px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    isActive(item)
                      ? "bg-blue-50 text-blue-700"
                      : "text-gray-600 hover:bg-gray-100 hover:text-gray-900"
                  }`}
                >
                  {item.label}
                </a>
              ))}
            </>
          )}
        </nav>
        <div className="px-4 py-3 border-t border-gray-200 text-xs text-gray-500">
          <a href="/chat" className="hover:text-gray-700">
            &larr; Back to Chat
          </a>
        </div>
      </aside>

      {/* Main Content — scrolls independently */}
      <main className="flex-1 overflow-y-auto h-full">
        <div className="max-w-4xl mx-auto px-6 py-8">
          {children}
        </div>
      </main>
    </div>
  );
}
