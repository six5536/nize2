"use client";

// @awa-component: NAV-Sidebar

import { useState, useEffect, useCallback } from "react";
import { useRouter } from "next/navigation";
import { SidebarToggle } from "./sidebar-toggle";
import { NewChatButton } from "./new-chat-button";
import { ConversationList } from "./conversation-list";
import { DeleteConfirmDialog } from "./delete-confirm-dialog";
import type { ConversationSummary } from "@/lib/types";
import { useAuthFetch } from "@/lib/auth-context";

interface SidebarProps {
  activeConversationId?: string;
  isCollapsed: boolean;
  onToggleCollapse: () => void;
  onMobileClose: () => void;
}

// @awa-impl: NAV-1_AC-2
// @awa-impl: NAV-1_AC-3
// @awa-impl: NAV-1_AC-4
export function Sidebar({ activeConversationId, isCollapsed, onToggleCollapse, onMobileClose }: SidebarProps) {
  const router = useRouter();
  const authFetch = useAuthFetch();
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [deleteTarget, setDeleteTarget] = useState<ConversationSummary | null>(null);

  // Fetch conversations on mount
  useEffect(() => {
    fetchConversations();
  }, []);

  const fetchConversations = async () => {
    try {
      setIsLoading(true);
      const res = await authFetch("/conversations");
      if (res.ok) {
        const data = await res.json();
        setConversations(
          (data.items || []).map((c: { id: string; title: string; createdAt: string; updatedAt: string }) => ({
            ...c,
            createdAt: new Date(c.createdAt),
            updatedAt: new Date(c.updatedAt),
          })),
        );
      }
    } catch (error) {
      console.error("Failed to fetch conversations:", error);
    } finally {
      setIsLoading(false);
    }
  };

  // @awa-impl: NAV-1.1_AC-2
  // @awa-impl: NAV-1.1_AC-3
  const handleNewChat = useCallback(async () => {
    try {
      const res = await authFetch("/conversations", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ title: "New Chat" }),
      });
      if (res.ok) {
        const data = await res.json();
        // Optimistically add to list
        const newConversation: ConversationSummary = {
          id: data.id,
          title: data.title,
          createdAt: new Date(),
          updatedAt: new Date(),
        };
        setConversations((prev) => [newConversation, ...prev]);
        router.push(`/chat/${data.id}`);
        onMobileClose();
      }
    } catch (error) {
      console.error("Failed to create conversation:", error);
    }
  }, [router, onMobileClose, authFetch]);

  // @awa-impl: NAV-1.2_AC-3
  const handleSelectConversation = useCallback(
    (id: string) => {
      router.push(`/chat/${id}`);
      onMobileClose();
    },
    [router, onMobileClose],
  );

  // @awa-impl: NAV-3_AC-1
  const handleDeleteClick = useCallback((conversation: ConversationSummary) => {
    setDeleteTarget(conversation);
  }, []);

  // @awa-impl: NAV-3_AC-2
  // @awa-impl: NAV-3_AC-3
  // @awa-impl: NAV-3_AC-4
  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget) return;

    try {
      const res = await authFetch(`/conversations/${deleteTarget.id}`, { method: "DELETE" });
      if (res.ok) {
        // Remove from list
        const remaining = conversations.filter((c) => c.id !== deleteTarget.id);
        setConversations(remaining);

        // Handle navigation after deletion
        if (activeConversationId === deleteTarget.id) {
          if (remaining.length > 0) {
            // @awa-impl: NAV-3_AC-3
            router.push(`/chat/${remaining[0].id}`);
          } else {
            // @awa-impl: NAV-3_AC-4
            // Create a new conversation if none remain
            handleNewChat();
          }
        }
      }
    } catch (error) {
      console.error("Failed to delete conversation:", error);
    } finally {
      setDeleteTarget(null);
    }
  }, [deleteTarget, conversations, activeConversationId, router, handleNewChat]);

  if (isCollapsed) {
    return (
      <div className="h-full bg-gray-50 border-r flex flex-col items-center py-4">
        <SidebarToggle isCollapsed={isCollapsed} onToggle={onToggleCollapse} />
        <button onClick={handleNewChat} className="mt-4 p-2 rounded-lg hover:bg-gray-200" title="New Chat">
          <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>
    );
  }

  return (
    <div className="h-full bg-gray-50 border-r flex flex-col">
      {/* Header */}
      <div className="p-4 border-b flex items-center justify-between">
        <h2 className="font-semibold text-gray-700">Chats</h2>
        <SidebarToggle isCollapsed={isCollapsed} onToggle={onToggleCollapse} />
      </div>

      {/* New Chat Button */}
      <div className="p-2">
        <NewChatButton onNewChat={handleNewChat} />
      </div>

      {/* Conversation List */}
      <div className="flex-1 overflow-y-auto">{isLoading ? <div className="p-4 text-sm text-gray-500">Loading...</div> : <ConversationList conversations={conversations} activeId={activeConversationId} onSelect={handleSelectConversation} onDelete={handleDeleteClick} />}</div>

      {/* Settings Link */}
      <div className="p-4 border-t">
        <button
          onClick={() => {
            router.push("/settings");
            onMobileClose();
          }}
          className="w-full flex items-center px-3 py-2 text-sm text-gray-700 hover:bg-gray-200 rounded-lg"
        >
          <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
          Settings
        </button>
      </div>

      {/* Delete Confirmation Dialog */}
      <DeleteConfirmDialog isOpen={!!deleteTarget} conversationTitle={deleteTarget?.title || ""} onConfirm={handleDeleteConfirm} onCancel={() => setDeleteTarget(null)} />
    </div>
  );
}
