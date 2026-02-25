"use client";

// @awa-component: DEV-RawChatTab

import type { ChatStateData } from "@/lib/types";
import { truncateObjectText } from "@/lib/dev-panel-context";

interface RawChatTabProps {
  chatState: ChatStateData | null;
}

// @awa-impl: DEV-2_AC-1
// @awa-impl: DEV-2_AC-2
// @awa-impl: DEV-2_AC-3
// @awa-impl: DEV-2_AC-4
export function RawChatTab({ chatState }: RawChatTabProps) {
  if (!chatState) {
    return <div className="flex items-center justify-center h-32 text-gray-400 text-sm">No active chat</div>;
  }

  // Truncate text content in messages to prevent excessive display
  const truncatedState = truncateObjectText(chatState);

  return (
    <div className="space-y-2">
      <div className="text-xs text-gray-400 mb-2">Chat State</div>
      <pre className="bg-gray-800 p-3 rounded text-xs overflow-x-auto">
        <code className="text-green-400">{JSON.stringify(truncatedState, null, 2)}</code>
      </pre>
    </div>
  );
}
