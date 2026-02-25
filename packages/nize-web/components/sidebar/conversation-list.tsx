"use client";

// @awa-component: NAV-ConversationList

import { ConversationItem } from "./conversation-item";
import type { ConversationSummary } from "@/lib/types";

interface ConversationListProps {
  conversations: ConversationSummary[];
  activeId?: string;
  onSelect: (id: string) => void;
  onDelete: (conversation: ConversationSummary) => void;
}

// @awa-impl: NAV-1.2_AC-1
// @awa-impl: NAV-1.2_AC-5
export function ConversationList({ conversations, activeId, onSelect, onDelete }: ConversationListProps) {
  if (conversations.length === 0) {
    return <div className="p-4 text-sm text-gray-500 text-center">No conversations yet</div>;
  }

  return (
    <div className="py-2">
      {conversations.map((conversation) => (
        <ConversationItem key={conversation.id} conversation={conversation} isActive={conversation.id === activeId} onSelect={() => onSelect(conversation.id)} onDelete={() => onDelete(conversation)} />
      ))}
    </div>
  );
}
