"use client";

// @zen-component: DEV-DevPanelContext

import { createContext, useContext } from "react";
import type { ChatStateData } from "./types";

// @zen-impl: DEV-3_AC-4
// @zen-impl: TRC-5_AC-2, TRC-5_AC-4
export interface DevPanelContextValue {
  isExpanded: boolean;
  setIsExpanded: (expanded: boolean) => void;
  activeTab: string;
  setActiveTab: (tab: string) => void;
  chatState: ChatStateData | null;
  setChatState: (state: ChatStateData | null) => void;
  panelWidth: number;
  setPanelWidth: (width: number) => void;
  panelHeight: number;
  setPanelHeight: (height: number) => void;
  // @zen-impl: TRC-5_AC-4
  isAdmin: boolean;
  setIsAdmin: (isAdmin: boolean) => void;
  // @zen-impl: TRC-5_AC-2, TRC-5_AC-5
  conversationId: string | null;
  setConversationId: (id: string | null) => void;
  // Key that changes when trace SSE should reconnect (e.g., new message sent)
  traceKey: number;
  incrementTraceKey: () => void;
}

export const DevPanelContext = createContext<DevPanelContextValue | null>(null);

export function useDevPanel(): DevPanelContextValue {
  const context = useContext(DevPanelContext);
  if (!context) {
    if (process.env.NODE_ENV === "development") {
      console.warn("useDevPanel must be used within DevPanelProvider");
    }
    // Return noop implementation for production safety
    return {
      isExpanded: false,
      setIsExpanded: () => {},
      activeTab: "chat-trace",
      setActiveTab: () => {},
      chatState: null,
      setChatState: () => {},
      panelWidth: 400,
      setPanelWidth: () => {},
      panelHeight: 300,
      setPanelHeight: () => {},
      isAdmin: false,
      setIsAdmin: () => {},
      conversationId: null,
      setConversationId: () => {},
      traceKey: 0,
      incrementTraceKey: () => {},
    };
  }
  return context;
}

/**
 * Truncates text content to prevent excessive display length
 * @param text - Text to truncate
 * @param maxLength - Maximum length before truncation (default: 200)
 * @returns Truncated text with ellipsis if needed
 */
export function truncateText(text: string, maxLength: number = 200): string {
  if (text.length <= maxLength) {
    return text;
  }
  return text.slice(0, maxLength) + "...";
}

/**
 * Recursively truncates text fields in objects for JSON display
 * @param obj - Object to process
 * @param maxLength - Maximum length for text fields
 * @returns Object with truncated text fields
 */
export function truncateObjectText(obj: unknown, maxLength: number = 200): unknown {
  if (typeof obj === "string") {
    return truncateText(obj, maxLength);
  }

  if (Array.isArray(obj)) {
    return obj.map((item) => truncateObjectText(item, maxLength));
  }

  if (obj && typeof obj === "object") {
    const result: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(obj)) {
      result[key] = truncateObjectText(value, maxLength);
    }
    return result;
  }

  return obj;
}
