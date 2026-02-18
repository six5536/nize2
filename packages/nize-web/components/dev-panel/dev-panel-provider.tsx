"use client";

// @zen-component: DEV-DevPanelProvider

import { ReactNode, useState, useEffect, useCallback } from "react";
import { DevPanelContext } from "@/lib/dev-panel-context";
import type { ChatStateData } from "@/lib/types";
import { DEV_PANEL_STORAGE_KEY, DEFAULT_PANEL_WIDTH, DEFAULT_PANEL_HEIGHT } from "@/lib/types";

interface DevPanelProviderProps {
  children: ReactNode;
}

// @zen-impl: DEV-1_AC-1
// @zen-impl: DEV-1_AC-2
// @zen-impl: TRC-5_AC-4
export function DevPanelProvider({ children }: DevPanelProviderProps) {
  // Only render dev panel in development
  const isDevelopment = process.env.NODE_ENV === "development";

  const [isExpanded, setIsExpanded] = useState(false);
  const [activeTab, setActiveTab] = useState("chat-trace");
  const [chatState, setChatState] = useState<ChatStateData | null>(null);
  const [panelWidth, setPanelWidth] = useState(DEFAULT_PANEL_WIDTH);
  const [panelHeight, setPanelHeight] = useState(DEFAULT_PANEL_HEIGHT);
  // @zen-impl: TRC-5_AC-4 - Track admin status
  const [isAdmin, setIsAdmin] = useState(false);
  // @zen-impl: TRC-5_AC-2, TRC-5_AC-5 - Track current conversation
  const [conversationId, setConversationId] = useState<string | null>(null);
  // Key that changes when trace SSE should reconnect (e.g., new message sent)
  const [traceKey, setTraceKey] = useState(0);
  const incrementTraceKey = useCallback(() => setTraceKey((k) => k + 1), []);

  // @zen-impl: DEV-1_AC-7
  // Load state from localStorage on mount
  useEffect(() => {
    if (!isDevelopment) return;

    try {
      const stored = localStorage.getItem(DEV_PANEL_STORAGE_KEY);
      if (stored) {
        const state = JSON.parse(stored);
        setIsExpanded(state.isExpanded ?? false);
        setPanelWidth(state.panelWidth ?? DEFAULT_PANEL_WIDTH);
        setPanelHeight(state.panelHeight ?? DEFAULT_PANEL_HEIGHT);
      }
    } catch {
      // Ignore parse errors
    }
  }, [isDevelopment]);

  // @zen-impl: DEV-1_AC-7
  // Save state to localStorage when it changes
  useEffect(() => {
    if (!isDevelopment) return;

    try {
      localStorage.setItem(DEV_PANEL_STORAGE_KEY, JSON.stringify({ isExpanded, panelWidth, panelHeight }));
    } catch {
      // Ignore storage errors
    }
  }, [isExpanded, panelWidth, panelHeight, isDevelopment]);

  // In production, just render children without dev panel
  if (!isDevelopment) {
    return <>{children}</>;
  }

  return (
    <DevPanelContext.Provider
      value={{
        isExpanded,
        setIsExpanded,
        activeTab,
        setActiveTab,
        chatState,
        setChatState,
        panelWidth,
        setPanelWidth,
        panelHeight,
        setPanelHeight,
        isAdmin,
        setIsAdmin,
        conversationId,
        setConversationId,
        traceKey,
        incrementTraceKey,
      }}
    >
      {children}
    </DevPanelContext.Provider>
  );
}
