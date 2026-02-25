"use client";

// @awa-component: NAV-ChatLayout

import { useState, useEffect } from "react";
import { Sidebar } from "@/components/sidebar/sidebar";
import { SIDEBAR_STORAGE_KEY, type SidebarState } from "@/lib/types";

interface ChatLayoutProps {
  children: React.ReactNode;
  conversationId?: string;
}

// @awa-impl: NAV-1_AC-1
// @awa-impl: NAV-1_AC-5
export function ChatLayout({ children, conversationId }: ChatLayoutProps) {
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [isMobileOpen, setIsMobileOpen] = useState(false);

  // Load collapsed state from localStorage
  useEffect(() => {
    try {
      const stored = localStorage.getItem(SIDEBAR_STORAGE_KEY);
      if (stored) {
        const state: SidebarState = JSON.parse(stored);
        setIsCollapsed(state.isCollapsed);
      }
    } catch {
      // Ignore parse errors
    }
  }, []);

  // Save collapsed state to localStorage
  const handleToggleCollapse = () => {
    const newState = !isCollapsed;
    setIsCollapsed(newState);
    try {
      localStorage.setItem(SIDEBAR_STORAGE_KEY, JSON.stringify({ isCollapsed: newState }));
    } catch {
      // Ignore storage errors
    }
  };

  // Close mobile sidebar when conversation changes
  useEffect(() => {
    setIsMobileOpen(false);
  }, [conversationId]);

  return (
    <div className="flex h-screen">
      {/* Mobile overlay */}
      {isMobileOpen && <div className="fixed inset-0 z-40 bg-black/50 md:hidden" onClick={() => setIsMobileOpen(false)} />}

      {/* Sidebar */}
      <div
        className={`
          ${isCollapsed ? "w-0 md:w-16" : "w-64"}
          ${isMobileOpen ? "translate-x-0" : "-translate-x-full md:translate-x-0"}
          fixed md:relative z-50 h-full transition-all duration-200 ease-in-out
        `}
      >
        <Sidebar activeConversationId={conversationId} isCollapsed={isCollapsed} onToggleCollapse={handleToggleCollapse} onMobileClose={() => setIsMobileOpen(false)} />
      </div>

      {/* Main content */}
      <div className="flex-1 flex flex-col min-w-0">
        {/* Mobile menu button */}
        <button onClick={() => setIsMobileOpen(true)} className="md:hidden fixed top-4 left-4 z-30 p-2 rounded-lg bg-white shadow-md border" aria-label="Open sidebar">
          <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>

        {children}
      </div>
    </div>
  );
}
