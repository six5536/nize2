"use client";

// @zen-component: NAV-NewChatButton

interface NewChatButtonProps {
  onNewChat: () => void;
  disabled?: boolean;
}

// @zen-impl: NAV-1.1_AC-1
// @zen-impl: NAV-1.1_AC-2
export function NewChatButton({ onNewChat, disabled }: NewChatButtonProps) {
  return (
    <button onClick={onNewChat} disabled={disabled} className="w-full flex items-center gap-2 px-4 py-2 rounded-lg border border-gray-300 hover:bg-gray-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
      <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
      </svg>
      <span className="text-sm font-medium">New Chat</span>
    </button>
  );
}
