// @zen-component: PLAN-012-TabView
import { useState, type ReactNode } from "react";

/** Tab definition for the generic TabView component. */
export interface TabDefinition {
  /** Unique tab identifier. */
  id: string;
  /** Display label. */
  label: string;
  /** Content rendered in the tab panel. */
  content: ReactNode;
  /** When true the tab is visible but not selectable. */
  disabled?: boolean;
}

export interface TabViewProps {
  tabs: TabDefinition[];
  /** If omitted the first tab is active. */
  defaultTab?: string;
}

/**
 * Generic n-tab component.
 *
 * All panels stay mounted (hidden via `display:none`) so that heavy children
 * such as iframes preserve their state across tab switches.
 */
// @zen-impl: PLAN-012-5.1
export function TabView({ tabs, defaultTab }: TabViewProps) {
  const [activeTab, setActiveTab] = useState(defaultTab ?? tabs[0]?.id);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      {/* Tab bar */}
      <nav
        role="tablist"
        style={{
          display: "flex",
          gap: "0",
          borderBottom: "1px solid #ddd",
          fontFamily: "system-ui, sans-serif",
          flexShrink: 0,
        }}
      >
        {tabs.map((tab) => (
          <button
            key={tab.id}
            role="tab"
            aria-selected={activeTab === tab.id}
            aria-controls={`panel-${tab.id}`}
            onClick={() => !tab.disabled && setActiveTab(tab.id)}
            disabled={tab.disabled}
            style={{
              padding: "0.5rem 1rem",
              border: "none",
              borderBottom: activeTab === tab.id ? "2px solid #333" : "2px solid transparent",
              background: "none",
              fontWeight: activeTab === tab.id ? 600 : 400,
              cursor: tab.disabled ? "not-allowed" : "pointer",
              opacity: tab.disabled ? 0.5 : 1,
            }}
          >
            {tab.label}
          </button>
        ))}
      </nav>

      {/* Panels — all stay mounted, only one visible */}
      {tabs.map((tab) => (
        <div
          key={tab.id}
          id={`panel-${tab.id}`}
          role="tabpanel"
          style={{
            flex: 1,
            display: activeTab === tab.id ? "flex" : "none",
            flexDirection: "column",
            overflow: "auto",
          }}
        >
          {tab.content}
        </div>
      ))}
    </div>
  );
}
