// @zen-component: CHAT-ScrollHook

import { useCallback, useRef, useEffect, type RefObject } from "react";

interface UseChatScrollOptions {
  /** Number of messages currently in the chat */
  messagesLength: number;
  /** Whether the chat is currently loading/streaming */
  isLoading: boolean;
  /** Content of the last message (for detecting streaming updates) */
  lastMessageContent: string;
  /** Role of the last message */
  lastMessageRole?: "user" | "assistant" | "system";
  /** Conversation ID (to reset scroll state on conversation change) */
  conversationId: string;
}

interface UseChatScrollReturn {
  /** Ref to attach to the messages container */
  containerRef: RefObject<HTMLDivElement | null>;
  /** Handler for scroll events */
  handleScroll: () => void;
  /** Reset scroll state (call when conversation changes) */
  resetScrollState: () => void;
}

/**
 * Hook to manage chat scroll behavior
 * - Auto-scrolls to bottom on new messages
 * - Respects user manual scroll (stops auto-scroll when user scrolls up)
 * - Uses smooth scroll for user messages, instant for streaming
 */
export function useChatScroll({ messagesLength, isLoading, lastMessageContent, lastMessageRole, conversationId }: UseChatScrollOptions): UseChatScrollReturn {
  const containerRef = useRef<HTMLDivElement>(null);
  const isUserScrolledRef = useRef(false);
  const previousMessagesLengthRef = useRef(messagesLength);
  const isInitialLoadRef = useRef(true);
  const previousConversationIdRef = useRef(conversationId);
  const smoothScrollActiveRef = useRef(false);

  // Check if user is scrolled to the bottom
  const checkIfAtBottom = useCallback(() => {
    if (!containerRef.current) return false;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    // Consider "at bottom" if within 100px of the bottom
    return scrollHeight - scrollTop - clientHeight < 100;
  }, []);

  // Handle scroll events to track user scroll position
  const handleScroll = useCallback(() => {
    isUserScrolledRef.current = !checkIfAtBottom();
  }, [checkIfAtBottom]);

  // Helper to scroll to bottom
  const scrollToBottom = useCallback((behavior: ScrollBehavior = "instant") => {
    if (!containerRef.current) return;
    containerRef.current.scrollTo({
      top: containerRef.current.scrollHeight,
      behavior,
    });
  }, []);

  // Reset state when conversation changes
  const resetScrollState = useCallback(() => {
    isInitialLoadRef.current = true;
    previousMessagesLengthRef.current = 0;
    isUserScrolledRef.current = false;
  }, []);

  // Handle conversation changes
  useEffect(() => {
    if (previousConversationIdRef.current !== conversationId) {
      resetScrollState();
      previousConversationIdRef.current = conversationId;
    }
  }, [conversationId, resetScrollState]);

  // Auto-scroll effect when message count changes
  useEffect(() => {
    if (!containerRef.current) return;

    const newMessageAdded = messagesLength > previousMessagesLengthRef.current;

    if (newMessageAdded) {
      // For initial load, scroll instantly to bottom
      if (isInitialLoadRef.current && messagesLength > 0) {
        scrollToBottom("instant");
        isUserScrolledRef.current = false;
        isInitialLoadRef.current = false;
      }
      // User message - scroll smoothly for nice UX
      else if (lastMessageRole === "user") {
        smoothScrollActiveRef.current = true;
        scrollToBottom("smooth");
        isUserScrolledRef.current = false;
        setTimeout(() => {
          smoothScrollActiveRef.current = false;
        }, 400);
      }
      // Assistant message - scroll instantly to avoid glitch during streaming
      else if (!isUserScrolledRef.current) {
        scrollToBottom("instant");
        isUserScrolledRef.current = false;
      }

      previousMessagesLengthRef.current = messagesLength;
    }
  }, [messagesLength, lastMessageRole, scrollToBottom]);

  // Auto-scroll when streaming content updates (message count stays same but content grows)
  // Use instant scroll to keep up with rapid updates, but not during smooth scroll animation
  useEffect(() => {
    if (isLoading && !isUserScrolledRef.current && !smoothScrollActiveRef.current) {
      scrollToBottom("instant");
    }
  }, [isLoading, lastMessageContent, scrollToBottom]);

  return {
    containerRef,
    handleScroll,
    resetScrollState,
  };
}
