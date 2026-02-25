"use client";

// @awa-component: DEV-TabBar

import type { TabDefinition } from "@/lib/types";

interface TabBarProps {
  tabs: TabDefinition[];
  activeTab: string;
  onTabChange: (tab: string) => void;
}

// @awa-impl: DEV-1.1_AC-1
// @awa-impl: DEV-1.1_AC-2
// @awa-impl: DEV-1.1_AC-3
export function TabBar({ tabs, activeTab, onTabChange }: TabBarProps) {
  return (
    <div className="shrink-0 border-b border-gray-700">
      <div className="flex gap-1 px-4">
        {tabs.map((tab) => (
          <button key={tab.id} onClick={() => onTabChange(tab.id)} className={`px-4 py-2 text-sm font-medium transition-colors ${activeTab === tab.id ? "text-blue-400 border-b-2 border-blue-400" : "text-gray-400 hover:text-gray-200"}`}>
            {tab.label}
          </button>
        ))}
      </div>
    </div>
  );
}
