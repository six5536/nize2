"use client";

// @zen-component: NAV-ConversationItem

import { useState } from "react";
import { formatDistanceToNow } from "date-fns";
import type { ConversationSummary } from "@/lib/types";

interface ConversationItemProps {
  conversation: ConversationSummary;
  isActive: boolean;
  onSelect: () => void;
  onDelete: () => void;
}

// @zen-impl: NAV-1.2_AC-2
// @zen-impl: NAV-1.2_AC-3
// @zen-impl: NAV-1.2_AC-4
// @zen-impl: NAV-3_AC-1
export function ConversationItem({ conversation, isActive, onSelect, onDelete }: ConversationItemProps) {
  const [isHovered, setIsHovered] = useState(false);

  // @zen-impl: NAV-1.2_AC-2
  const relativeTime = formatDistanceToNow(conversation.updatedAt, { addSuffix: true });

  const handleDeleteClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete();
  };

  return (
    <div
      onClick={onSelect}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      className={`
        px-3 py-2 mx-2 rounded-lg cursor-pointer transition-colors relative group
        ${isActive ? "bg-blue-100 text-blue-900" : "hover:bg-gray-100"}
      `}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium truncate">{conversation.title}</p>
          <p className="text-xs text-gray-500 truncate">{relativeTime}</p>
        </div>

        {/* Delete button - shows on hover */}
        {isHovered && (
          <button onClick={handleDeleteClick} className="p-1 rounded hover:bg-gray-200 text-gray-500 hover:text-red-600 transition-colors" aria-label="Delete conversation">
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}
