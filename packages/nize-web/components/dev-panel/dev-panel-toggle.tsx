"use client";

// @awa-component: DEV-DevPanelToggle

interface DevPanelToggleProps {
  isExpanded: boolean;
  onToggle: () => void;
}

// @awa-impl: DEV-1_AC-5
// @awa-impl: DEV-1_AC-6
export function DevPanelToggle({ isExpanded, onToggle }: DevPanelToggleProps) {
  return (
    <button onClick={onToggle} className="absolute top-4 left-2 p-2 bg-gray-800 text-white rounded-md hover:bg-gray-700 transition-colors z-10" aria-label={isExpanded ? "Collapse dev panel" : "Expand dev panel"} title={isExpanded ? "Collapse dev panel" : "Expand dev panel"}>
      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        {/* Horizontal chevron (points right when expanded to collapse, left when collapsed to expand) */}
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d={isExpanded ? "M9 5l7 7-7 7" : "M15 19l-7-7 7-7"} />
      </svg>
    </button>
  );
}
