"use client";

// @zen-component: CHAT-ChatContainer
// @zen-impl: NAV-1.2_AC-2, NAV-2_AC-1, NAV-2.1_AC-1
// @zen-impl: DEV-2_AC-2

import { useChat } from "@ai-sdk/react";
import { DefaultChatTransport, type UIMessage } from "ai";
import { useParams, useRouter } from "next/navigation";
import { useEffect, useState, useMemo, useCallback, useRef } from "react";
import { ChatLayout, ChatHeader, ChatInput, ChatUpload, MessageBubble, ThinkingBubble, EmptyState, useChatScroll, useFileUpload, useChatSubmit } from "@/components/chat";
import { useDevPanel } from "@/lib/dev-panel-context";
import { useAuth, useAuthFetch } from "@/lib/auth-context";
import { apiUrl } from "@/lib/api";
import { extractTextContent } from "@/lib/message-parts";

// Check localStorage directly for auth (handles race condition after login redirect)
function hasStoredAuth(): boolean {
  if (typeof window === "undefined") return false;
  return !!localStorage.getItem("nize_user");
}

export default function ConversationChatPage() {
  const { user, isLoading: authLoading, isAuthenticated, logout } = useAuth();
  const authFetch = useAuthFetch();
  const router = useRouter();
  const params = useParams();
  const conversationId = params.conversationId as string;

  // Create transport with dynamic body containing conversationId - cookies sent automatically
  const transport = useMemo(
    () =>
      new DefaultChatTransport({
        api: apiUrl("/chat"),
        body: () => ({ conversationId }),
        credentials: "include", // Send httpOnly cookies
      }),
    [conversationId],
  );

  const { messages, sendMessage, status, setMessages } = useChat({ transport });
  const [input, setInput] = useState("");

  const isLoading = status === "submitted" || status === "streaming";
  const lastMessage = messages[messages.length - 1];
  const lastMessageContent = lastMessage ? extractTextContent(lastMessage) : "";

  // Custom hooks for scroll, upload, and chat submit
  const { containerRef, handleScroll, resetScrollState } = useChatScroll({
    messagesLength: messages.length,
    isLoading,
    lastMessageContent,
    lastMessageRole: lastMessage?.role as "user" | "assistant" | undefined,
    conversationId,
  });

  const { uploading, uploadMessage, handleUpload } = useFileUpload();
  const { handleChatSubmit } = useChatSubmit({ input, setInput, sendMessage, setMessages });

  // Reset messages when conversation changes
  useEffect(() => {
    setMessages([]);
    resetScrollState();
  }, [conversationId, setMessages, resetScrollState]);

  // @zen-impl: DEV-2_AC-2
  // @zen-impl: TRC-5_AC-2 - Wire conversationId to dev panel
  const { setChatState, setConversationId, setIsAdmin, incrementTraceKey } = useDevPanel();
  useEffect(() => {
    setChatState({ messages, isLoading, error: null, input });
  }, [messages, isLoading, input, setChatState]);

  // Trigger trace SSE reconnection when chat becomes loading (new message sent)
  const prevIsLoadingRef = useRef(false);
  useEffect(() => {
    if (isLoading && !prevIsLoadingRef.current) {
      incrementTraceKey();
    }
    prevIsLoadingRef.current = isLoading;
  }, [isLoading, incrementTraceKey]);

  // @zen-impl: TRC-5_AC-2 - Set conversation ID for trace tab
  useEffect(() => {
    setConversationId(conversationId);
    return () => setConversationId(null);
  }, [conversationId, setConversationId]);

  // @zen-impl: TRC-5_AC-4 - Set admin status from user roles
  useEffect(() => {
    setIsAdmin(user?.roles?.includes("admin") ?? false);
  }, [user?.roles, setIsAdmin]);

  // Redirect to login if not authenticated
  useEffect(() => {
    if (!authLoading && !isAuthenticated && !hasStoredAuth()) {
      router.replace("/login");
    }
  }, [router, authLoading, isAuthenticated]);

  // Load conversation history from Hono API
  useEffect(() => {
    const hasAuth = isAuthenticated || hasStoredAuth();
    if (!conversationId || !hasAuth || authLoading) return;

    const loadHistory = async () => {
      try {
        const res = await authFetch(`/conversations/${conversationId}`);
        if (!res.ok) {
          if (res.status === 404 || res.status === 403 || res.status === 401) {
            router.replace("/chat");
          }
          return;
        }
        const data = (await res.json()) as { messages?: UIMessage[] };
        if (Array.isArray(data.messages)) {
          setMessages(data.messages);
        }
      } catch {
        // Ignore history load errors
      }
    };
    loadHistory();
  }, [user?.id, conversationId, isAuthenticated, authLoading, setMessages, router, authFetch]);

  // @zen-impl: AUTH-3_AC-1
  const handleLogout = useCallback(async () => {
    await logout();
    router.push("/login");
  }, [logout, router]);

  const handleInputChange = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    setInput(event.target.value);
  }, []);

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
      <div className="flex h-full flex-col">
        <ChatHeader userName={user?.email} onLogout={handleLogout} />

        <div ref={containerRef} onScroll={handleScroll} className="flex-1 overflow-y-auto p-6" style={{ overscrollBehavior: "contain", overflowAnchor: "none" }}>
          <div className="mx-auto max-w-3xl space-y-4">
            <ChatUpload onUpload={handleUpload} uploading={uploading} uploadMessage={uploadMessage} />
            {messages.length === 0 ? <EmptyState /> : messages.map((message, index) => <MessageBubble key={message.id} message={message} isStreaming={status === "streaming" && index === messages.length - 1} />)}
            {status === "submitted" && <ThinkingBubble />}
          </div>
        </div>

        <ChatInput value={input} onChange={handleInputChange} onSubmit={handleChatSubmit} isLoading={isLoading} />
      </div>
    </ChatLayout>
  );
}
