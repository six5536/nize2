"use client";

// @zen-component: CHAT-ChatRedirect
// @zen-impl: NAV-1_AC-1

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { ChatLayout } from "@/components/chat/chat-layout";
import { useAuth, useAuthFetch } from "@/lib/auth-context";

// Check localStorage directly for auth (handles race condition after login redirect)
function hasStoredAuth(): boolean {
  if (typeof window === "undefined") return false;
  return !!localStorage.getItem("nize_access_token");
}

// Main /chat page redirects to the most recent conversation or creates one
export default function ChatPage() {
  const { isLoading: authLoading, isAuthenticated } = useAuth();
  const authFetch = useAuthFetch();
  const router = useRouter();
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    if (authLoading) return;

    // Check both context state and localStorage (handles post-login redirect race)
    const hasAuth = isAuthenticated || hasStoredAuth();
    if (!hasAuth) {
      router.replace("/login");
      return;
    }

    // Check for existing conversations or create one
    const initializeChat = async () => {
      try {
        const res = await authFetch("/conversations");
        if (res.ok) {
          const data = await res.json();
          if (data.items && data.items.length > 0) {
            // Redirect to most recent conversation
            router.replace(`/chat/${data.items[0].id}`);
            return;
          }
        }

        // No conversations exist, create one
        const createRes = await authFetch("/conversations", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ title: "New Chat" }),
        });
        if (createRes.ok) {
          const newConv = await createRes.json();
          router.replace(`/chat/${newConv.id}`);
          return;
        }
      } catch (error) {
        console.error("Failed to initialize chat:", error);
      }
      setIsLoading(false);
    };

    initializeChat();
  }, [authLoading, isAuthenticated, router, authFetch]);

  if (authLoading || isLoading) {
    return (
      <ChatLayout>
        <div className="flex h-full items-center justify-center text-gray-500">Loading...</div>
      </ChatLayout>
    );
  }

  return (
    <ChatLayout>
      <div className="flex h-full items-center justify-center text-gray-500">Redirecting...</div>
    </ChatLayout>
  );
}
