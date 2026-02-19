// @zen-component: PLAN-027-ChatService

import { streamText, convertToModelMessages, stepCountIs, type UIMessage, type ToolSet } from "ai";
import type { ChatConfig, ChatRequest, CompactMessage } from "./types";
import { getChatModel, getProviderFromSpec } from "./model-registry";
import type { GetChatModelOptions } from "./model-registry";
import { maybeCompact } from "./compaction";
import { createProxyFetch } from "./proxy-fetch";
import { createMcpSession } from "./mcp-client";

// ============================================================================
// Helpers
// ============================================================================

/** Extract text content from a UIMessage (v6 uses parts array) */
function getMessageText(message: UIMessage): string {
  if (message.parts && message.parts.length > 0) {
    return message.parts
      .filter((part): part is { type: "text"; text: string } => part.type === "text")
      .map((part) => part.text)
      .join("");
  }
  return (message as unknown as { content?: string }).content ?? "";
}

/** Convert UIMessages to CompactMessages for compaction (text-only) */
function toCompactMessages(messages: UIMessage[]): CompactMessage[] {
  return messages.map((msg) => ({
    role: msg.role as "user" | "assistant" | "system",
    content: getMessageText(msg),
  }));
}

// ============================================================================
// Title Generation
// ============================================================================

/**
 * Generate a short title for a conversation based on the first user message.
 */
// @zen-impl: PLAN-028-3.5
async function generateTitle(userMessage: string, modelSpec: string, modelOptions?: GetChatModelOptions): Promise<string> {
  const model = getChatModel(modelSpec, modelOptions);
  const result = await streamText({
    model,
    messages: [
      {
        role: "user",
        content: `Generate a brief, descriptive title (3-6 words) for a conversation that starts with this message. Return only the title, nothing else:\n\n"${userMessage}"`,
      },
    ],
  });

  let title = "";
  for await (const chunk of result.textStream) {
    title += chunk;
  }

  return title.trim().slice(0, 50) || "New Chat";
}

// ============================================================================
// Rust API Helpers
// ============================================================================

async function getOrCreateConversation(apiBaseUrl: string, cookie: string, conversationId?: string): Promise<{ id: string; title: string; isNew: boolean }> {
  if (conversationId) {
    // Validate conversation exists and belongs to user
    const res = await fetch(`${apiBaseUrl}/api/conversations/${conversationId}`, { headers: { cookie } });
    if (!res.ok) {
      throw new ConversationNotFoundError("Conversation not found");
    }
    const data = (await res.json()) as { id: string; title: string };
    return { id: data.id, title: data.title, isNew: false };
  }

  // Create new conversation
  const res = await fetch(`${apiBaseUrl}/api/conversations`, {
    method: "POST",
    headers: { "Content-Type": "application/json", cookie },
    body: JSON.stringify({ title: "New Chat" }),
  });
  if (!res.ok) {
    throw new Error(`Failed to create conversation: ${res.status}`);
  }
  const data = (await res.json()) as { id: string; title: string };
  return { id: data.id, title: data.title, isNew: true };
}

async function persistMessages(apiBaseUrl: string, cookie: string, conversationId: string, messages: UIMessage[]): Promise<void> {
  const res = await fetch(`${apiBaseUrl}/api/conversations/${conversationId}/messages`, {
    method: "PUT",
    headers: { "Content-Type": "application/json", cookie },
    body: JSON.stringify({ messages }),
  });
  if (!res.ok) {
    console.error(`Failed to persist messages: ${res.status}`);
  }
}

async function updateConversationTitle(apiBaseUrl: string, cookie: string, conversationId: string, title: string): Promise<void> {
  const res = await fetch(`${apiBaseUrl}/api/conversations/${conversationId}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json", cookie },
    body: JSON.stringify({ title }),
  });
  if (!res.ok) {
    console.error(`Failed to update title: ${res.status}`);
  }
}

// ============================================================================
// processChat
// ============================================================================

export interface ProcessChatResult {
  toUIMessageStreamResponse(): Response;
  conversationId: string;
  isNewConversation: boolean;
}

/**
 * Process a chat request: get/create conversation, compact history,
 * stream AI response, persist on finish.
 */
// @zen-impl: PLAN-028-3.5
// @zen-impl: PLAN-029-3.5
export async function processChat(request: ChatRequest, config: ChatConfig, apiBaseUrl: string, cookie: string, mcpBaseUrl?: string): Promise<ProcessChatResult> {
  const conversation = await getOrCreateConversation(apiBaseUrl, cookie, request.conversationId);

  const allMessages = request.messages;
  const lastMessage = allMessages[allMessages.length - 1];
  const userMessageText = getMessageText(lastMessage);

  // Check if title generation is needed
  const isFirstMessage = allMessages.filter((m) => m.role === "user").length === 1;
  const shouldGenerateTitle = isFirstMessage && conversation.title === "New Chat";

  // Compaction: flatten to text, run maybeCompact, decide message format
  const compactMessages = toCompactMessages(allMessages);
  const compactedState = maybeCompact(compactMessages, config.compactionMaxMessages);
  const wasCompacted = !!compactedState.summary;

  // Build model messages:
  // - If compacted: use text-only compacted messages
  // - Otherwise: use convertToModelMessages to preserve tool calls/results
  const modelMessages = wasCompacted ? compactedState.messages.filter((msg) => msg.content.trim() !== "").map((msg) => ({ role: msg.role, content: msg.content })) : await convertToModelMessages(allMessages);

  // Create proxy fetch and model options for the provider
  const providerType = getProviderFromSpec(config.modelName);
  const proxyFetch = createProxyFetch(apiBaseUrl, cookie, providerType);
  const modelOptions: GetChatModelOptions = {
    fetch: proxyFetch,
    baseUrls: config.baseUrls,
  };

  const model = getChatModel(config.modelName, modelOptions);

  // @zen-impl: PLAN-029-3.5 — create MCP session for tool calling
  let mcpClient: Awaited<ReturnType<typeof createMcpSession>> | null = null;
  let tools: ToolSet | undefined;

  if (config.toolsEnabled && mcpBaseUrl) {
    try {
      console.log("[mcp] Creating MCP session...");
      mcpClient = await createMcpSession(apiBaseUrl, cookie, mcpBaseUrl);
      console.log("[mcp] Session created, fetching tools...");
      tools = await mcpClient.tools();
      console.log(`[mcp] Got ${Object.keys(tools).length} tools`);
    } catch (err) {
      console.error("Failed to create MCP session, continuing without tools:", err);
      mcpClient = null;
      tools = undefined;
    }
  }

  // When tools are enabled, prepend the tools system prompt
  const systemMessages = config.toolsEnabled && tools && config.toolsSystemPrompt ? [{ role: "system" as const, content: config.toolsSystemPrompt }] : [];

  const result = streamText({
    model,
    messages: [...systemMessages, ...modelMessages],
    temperature: config.temperature,
    ...(tools ? { tools, stopWhen: stepCountIs(config.toolsMaxSteps) } : {}),
    onStepFinish: ({ finishReason, toolCalls }) => {
      console.log(`[mcp] Step finished: reason=${finishReason}, toolCalls=${toolCalls?.length ?? 0}`);
    },
    onError: (error) => {
      console.error("Chat stream error:", error);
    },
  });

  // Consume stream to ensure it completes even if client disconnects
  result.consumeStream();

  return {
    toUIMessageStreamResponse: () =>
      result.toUIMessageStreamResponse({
        originalMessages: allMessages,
        onFinish: async ({ messages: finalMessages }) => {
          // Persist all messages via Rust API — do this BEFORE closing the
          // MCP client so the database is still healthy. Closing the MCP
          // session can trigger PGlite instability, so treat it as best-effort.
          try {
            await persistMessages(apiBaseUrl, cookie, conversation.id, finalMessages);
          } catch (err) {
            console.error("Failed to persist messages:", err);
          }

          // Close MCP client after messages are persisted
          if (mcpClient) {
            try {
              await mcpClient.close();
            } catch (err) {
              console.error("Failed to close MCP client:", err);
            }
          }

          // Generate title asynchronously for first message
          if (shouldGenerateTitle) {
            generateTitle(userMessageText, config.modelName, modelOptions)
              .then((title) => updateConversationTitle(apiBaseUrl, cookie, conversation.id, title))
              .catch((err) => console.error("Title generation failed:", err));
          }
        },
      }),
    conversationId: conversation.id,
    isNewConversation: conversation.isNew,
  };
}

// ============================================================================
// Errors
// ============================================================================

export class ConversationNotFoundError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ConversationNotFoundError";
  }
}
