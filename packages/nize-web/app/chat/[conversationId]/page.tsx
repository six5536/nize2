"use client";

// @zen-component: CHAT-ChatContainer
// Chat content area — left empty; chat messaging is not yet implemented.

import { useParams, useRouter } from "next/navigation";
import { useEffect } from "react";
import { ChatLayout } from "@/components/chat/chat-layout";
import { useAuth } from "@/lib/auth-context";

// Check localStorage directly for auth (handles race condition after login redirect)
function hasStoredAuth(): boolean {
  if (typeof window === "undefined") return false;
  return !!localStorage.getItem("nize_access_token");
}

export default function ConversationChatPage() {
  const { isLoading: authLoading, isAuthenticated } = useAuth();
  const router = useRouter();
  const params = useParams();
  const conversationId = params.conversationId as string;

  // Redirect to login if not authenticated
  useEffect(() => {
    if (!authLoading && !isAuthenticated && !hasStoredAuth()) {
      router.replace("/login");
    }
  }, [router, authLoading, isAuthenticated]);

  if (authLoading) {
    return (
      <ChatLayout conversationId={conversationId}>
        <div className="flex h-full items-center justify-center text-gray-500">Loading...</div>
      </ChatLayout>
    );
  }

  if (!isAuthenticated) return null;

  return (
    <ChatLayout conversationId={conversationId}>
      <div className="flex h-full flex-col items-center justify-center text-gray-400">
        <svg className="w-16 h-16 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
        </svg>
        <p className="text-lg font-medium">Chat coming soon</p>
        <p className="text-sm mt-1">Messaging is not yet available.</p>
      </div>
    </ChatLayout>
  );
}
