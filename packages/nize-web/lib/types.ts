// @zen-component: NAV-Types

// @zen-impl: NAV-2.1_AC-2
export interface ConversationSummary {
  id: string;
  title: string;
  createdAt: Date;
  updatedAt: Date;
}

// @zen-impl: NAV-1_AC-4
export interface SidebarState {
  isCollapsed: boolean;
}

export const SIDEBAR_STORAGE_KEY = "nize:sidebar";

// Dev Panel Types
export interface DevPanelState {
  isExpanded: boolean;
  width: number;
}

export interface ChatStateData {
  messages: unknown[];
  isLoading: boolean;
  error: Error | null;
  input: string;
}

export interface TabDefinition {
  id: string;
  label: string;
}

export const DEV_PANEL_STORAGE_KEY = "nize:dev-panel";

// Dev Panel Width Constants
export const MIN_PANEL_WIDTH = 300;
export const DEFAULT_PANEL_WIDTH = 384; // 96 in tailwind (w-96)
export const MAX_PANEL_WIDTH_RATIO = 2 / 3; // 2/3 of screen width

// Dev Panel Height Constants
export const MIN_PANEL_HEIGHT = 200;
export const DEFAULT_PANEL_HEIGHT = 300;
export const MAX_PANEL_HEIGHT_RATIO = 2 / 3; // 2/3 of screen height
