"use client";

// @awa-component: DEV-DevPanel

import { useState, useCallback } from "react";
import { useDevPanel } from "@/lib/dev-panel-context";
import { DevPanelToggle } from "./dev-panel-toggle";
import { ResizeHandle } from "./resize-handle";
import dynamic from "next/dynamic";

// Dynamically import tab components to ensure tree-shaking in production
const TabBar = dynamic(() => import("./tab-bar").then((mod) => ({ default: mod.TabBar })), {
  ssr: false,
});

const RawChatTab = dynamic(() => import("./tabs/raw-chat-tab").then((mod) => ({ default: mod.RawChatTab })), {
  ssr: false,
});

// @awa-impl: TRC-5_AC-1 - Chat Trace tab
const ChatTraceTab = dynamic(() => import("./tabs/chat-trace-tab").then((mod) => ({ default: mod.ChatTraceTab })), {
  ssr: false,
});

// @awa-impl: DEV-1_AC-3
// @awa-impl: DEV-1_AC-4
// @awa-impl: DEV-1_AC-7
// @awa-impl: DEV-1_AC-8
// @awa-impl: DEV-3_AC-1
// @awa-impl: DEV-3_AC-2
// @awa-impl: DEV-3_AC-3
// @awa-impl: DEV-3_AC-4
export function DevPanel() {
  const { isExpanded, setIsExpanded, activeTab, setActiveTab, chatState, panelWidth, setPanelWidth, panelHeight, setPanelHeight } = useDevPanel();

  // Track drag state to disable transitions during resize
  const [isDragging, setIsDragging] = useState(false);
  const handleDragStateChange = useCallback((dragging: boolean) => {
    setIsDragging(dragging);
  }, []);

  // Only render in development
  if (process.env.NODE_ENV !== "development") {
    return null;
  }

  // @awa-impl: TRC-5_AC-1 - Chat Trace as first tab
  const tabs = [
    { id: "chat-trace", label: "Chat Trace" },
    { id: "raw-chat", label: "Raw Chat" },
  ];

  return (
    <div
      data-dev-panel
      className={`bg-gray-900 text-gray-100 border-gray-700 shrink-0 relative ${isDragging ? "" : "transition-[width,height] duration-200 ease-in-out"} ${isExpanded ? "border-l lg:border-l border-t lg:border-t-0 w-full lg:w-auto lg:h-full" : "h-12 lg:h-full lg:w-12 border-t lg:border-t-0 lg:border-l"}`}
      style={
        isExpanded
          ? {
              // On mobile (flex-col), use height; on desktop (flex-row), use width
              height: `var(--panel-height, ${panelHeight}px)`,
              width: `var(--panel-width, ${panelWidth}px)`,
            }
          : undefined
      }
    >
      {/* CSS custom properties for responsive sizing */}
      <style jsx>{`
        @media (max-width: 1023px) {
          [data-dev-panel] {
            --panel-height: ${panelHeight}px;
            --panel-width: 100%;
            width: 100% !important;
          }
        }
        @media (min-width: 1024px) {
          [data-dev-panel] {
            --panel-width: ${panelWidth}px;
            --panel-height: 100%;
            height: 100% !important;
          }
        }
      `}</style>

      {/* Resize handles */}
      {isExpanded && (
        <>
          {/* Horizontal resize (for width on desktop) */}
          <div className="hidden lg:block">
            <ResizeHandle orientation="horizontal" onResize={setPanelWidth} currentSize={panelWidth} onDragStateChange={handleDragStateChange} />
          </div>
          {/* Vertical resize (for height on mobile) */}
          <div className="lg:hidden">
            <ResizeHandle orientation="vertical" onResize={setPanelHeight} currentSize={panelHeight} onDragStateChange={handleDragStateChange} />
          </div>
        </>
      )}

      <DevPanelToggle isExpanded={isExpanded} onToggle={() => setIsExpanded(!isExpanded)} />

      {isExpanded && (
        <div className="flex flex-col h-full">
          <div className="shrink-0 p-4 pl-12 border-b border-gray-700">
            <h2 className="text-lg font-semibold text-gray-100">Dev Panel</h2>
          </div>

          <TabBar tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab} />

          <div className="flex-1 overflow-auto p-4">
            {activeTab === "chat-trace" && <ChatTraceTab />}
            {activeTab === "raw-chat" && <RawChatTab chatState={chatState} />}
          </div>
        </div>
      )}

      {!isExpanded && (
        <div className="flex items-center justify-center h-full">
          <div className="lg:-rotate-90 text-xs text-gray-400 whitespace-nowrap">DEV</div>
        </div>
      )}
    </div>
  );
}
