"use client";

// @awa-component: NAV-SidebarToggle

interface SidebarToggleProps {
  isCollapsed: boolean;
  onToggle: () => void;
}

// @awa-impl: NAV-1_AC-2
// @awa-impl: NAV-1_AC-3
export function SidebarToggle({ isCollapsed, onToggle }: SidebarToggleProps) {
  return (
    <button onClick={onToggle} className="p-2 rounded-lg hover:bg-gray-200 transition-colors" aria-label={isCollapsed ? "Expand sidebar" : "Collapse sidebar"}>
      {isCollapsed ? (
        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      ) : (
        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
        </svg>
      )}
    </button>
  );
}
