// @zen-component: PLAN-027-HonoApp

import { Hono } from "hono";
import { processChat, ConversationNotFoundError } from "./chat-service";
import { fetchChatConfig } from "./chat-config";
import type { ChatRequest } from "./types";

/**
 * Hono app that handles chat requests.
 *
 * Mounted at `/api` basePath — expects POST /chat.
 * Auth is delegated to the Rust API (cookie forwarded on all backend calls).
 */
export const chatApp = new Hono().basePath("/api");

chatApp.post("/chat", async (c) => {
  const cookie = c.req.header("cookie") ?? "";

  // Resolve API base URL:
  // - NIZE_API_URL env var (explicit)
  // - NIZE_API_PORT env var (e.g. set by nize-web server.js)
  // - fallback to localhost:3001
  const apiBaseUrl = process.env.NIZE_API_URL ?? (process.env.NIZE_API_PORT ? `http://127.0.0.1:${process.env.NIZE_API_PORT}` : "http://127.0.0.1:3001");

  try {
    // Parse request body
    const body = (await c.req.json()) as ChatRequest;

    if (!body.messages || !Array.isArray(body.messages) || body.messages.length === 0) {
      return c.json({ error: "validation_error", message: "messages array is required" }, 400);
    }

    // Fetch config from Rust API
    const config = await fetchChatConfig(apiBaseUrl, cookie);

    // Process chat
    const result = await processChat(body, config, apiBaseUrl, cookie);

    // Return streaming response
    const response = result.toUIMessageStreamResponse();

    // Set conversation ID header for new conversations
    if (result.isNewConversation) {
      response.headers.set("X-Conversation-Id", result.conversationId);
    }

    return response;
  } catch (error) {
    if (error instanceof ConversationNotFoundError) {
      return c.json({ error: "not_found", message: error.message }, 404);
    }
    console.error("Chat error:", error instanceof Error ? error.stack : error);
    return c.json({ error: "internal_error", message: "Internal server error" }, 500);
  }
});
