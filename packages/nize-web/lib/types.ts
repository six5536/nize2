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
