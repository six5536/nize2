/**
 * Chat API contract for Nize.
 * Defines streaming chat endpoint with RAG context retrieval.
 *
 * Ported from ref project: submodules/nize/packages/api-types/src/chat.tsp
 */
import "@typespec/http";
import "@typespec/rest";
import "./API-NIZE-common.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;

namespace NizeApi.Chat;

// ============================================================================
// Request Models
// ============================================================================

/** UI message part — supports text, reasoning, tool calls, files, etc. */
model UIMessagePart {
  @doc("Part type (text, reasoning, file, step-start, dynamic-tool, or tool-{name})")
  type: string;

  @doc("Text content (for text/reasoning parts)")
  text?: string;

  @doc("Tool call ID (for tool parts)")
  toolCallId?: string;

  @doc("Tool name (for dynamic-tool parts)")
  toolName?: string;

  @doc("Tool state (for tool parts)")
  state?: string;
}

/** UI Message (AI SDK format) */
model UIMessage {
  @doc("Unique message ID")
  id: string;

  @doc("Message role")
  role: "user" | "assistant" | "system";

  @doc("Message parts array")
  parts: UIMessagePart[];

  @doc("Creation timestamp")
  createdAt?: string;
}

/** Send a chat message (AI SDK compatible format) */
model ChatRequest {
  @doc("Messages array (AI SDK UIMessage format)")
  messages: UIMessage[];

  @doc("Conversation ID (optional, creates new if not provided)")
  conversationId?: NizeApi.UUID;
}

// ============================================================================
// Response Models
// ============================================================================

/** Non-streaming chat response (demo) */
model ChatCompletionResponse {
  @doc("Generated response content")
  content: string;

  @doc("Conversation ID")
  conversationId: NizeApi.UUID;

  @doc("Message ID")
  messageId: NizeApi.UUID;
}

// ============================================================================
// Chat Routes
// ============================================================================

@route("/chat")
@tag("Chat")
interface ChatRoutes {
  /**
   * Send a chat message and receive response.
   * Demo: returns simple JSON response.
   */
  @post
  @summary("Send chat message")
  chat(
    @body body: ChatRequest,
  ): ChatCompletionResponse | NizeApi.UnauthorizedError | NizeApi.ValidationError;
}
