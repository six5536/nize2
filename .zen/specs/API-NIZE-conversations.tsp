/**
 * Conversations API contract for Nize.
 * Defines endpoints for managing chat conversations and messages.
 *
 * Ported from ref project: submodules/nize/packages/api-types/src/conversations.tsp
 */
import "@typespec/http";
import "@typespec/rest";
import "./API-NIZE-common.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;

namespace NizeApi.Conversations;

// ============================================================================
// Models
// ============================================================================

/** Conversation summary for list view */
model ConversationSummary {
  @doc("Conversation unique identifier")
  id: NizeApi.UUID;

  @doc("Conversation title")
  title: string;

  @doc("Creation timestamp")
  createdAt: NizeApi.DateTime;

  @doc("Last activity timestamp")
  updatedAt: NizeApi.DateTime;
}

/** Message part (AI SDK UIMessage format) */
model MessagePart {
  @doc("Part type")
  type: "text" | "tool-call" | "tool-result" | "reasoning";

  @doc("Text content (for text parts)")
  text?: string;
}

/** Chat message in UIMessage format */
model Message {
  @doc("Message unique identifier")
  id: NizeApi.UUID;

  @doc("Message role")
  role: "user" | "assistant" | "system";

  @doc("Message parts array (AI SDK UIMessage format)")
  parts: MessagePart[];

  @doc("Message creation timestamp")
  createdAt: NizeApi.DateTime;
}

/** Full conversation with messages */
model Conversation {
  @doc("Conversation unique identifier")
  id: NizeApi.UUID;

  @doc("Conversation title")
  title: string;

  @doc("Messages in the conversation")
  messages: Message[];

  @doc("Creation timestamp")
  createdAt: NizeApi.DateTime;

  @doc("Last activity timestamp")
  updatedAt: NizeApi.DateTime;
}

/** Create conversation request */
model CreateConversationRequest {
  @doc("Initial title (defaults to 'New Chat')")
  title?: string;
}

/** Update conversation request */
model UpdateConversationRequest {
  @doc("New conversation title")
  title: string;
}

/** Bulk save messages request (AI SDK UIMessage format) */
model BulkSaveMessagesRequest {
  @doc("Messages to save (AI SDK UIMessage format)")
  messages: NizeApi.Chat.UIMessage[];
}

// ============================================================================
// Conversations Routes
// ============================================================================

@route("/conversations")
@tag("Conversations")
interface ConversationsRoutes {
  /**
   * List all conversations for the authenticated user.
   * Sorted by most recent activity.
   */
  @get
  @summary("List conversations")
  list(
    ...NizeApi.PaginationParams,
  ): NizeApi.PaginatedResponse<ConversationSummary> | NizeApi.UnauthorizedError;

  /**
   * Create a new conversation.
   */
  @post
  @summary("Create conversation")
  create(@body body: CreateConversationRequest): {
    @statusCode statusCode: 201;
    @body body: ConversationSummary;
  } | NizeApi.ValidationError | NizeApi.UnauthorizedError;

  /**
   * Get a conversation by ID with all messages.
   */
  @get
  @route("/{id}")
  @summary("Get conversation")
  get(
    @path id: NizeApi.UUID,
  ): Conversation | NizeApi.NotFoundError | NizeApi.UnauthorizedError;

  /**
   * Update a conversation (e.g., title).
   */
  @patch
  @route("/{id}")
  @summary("Update conversation")
  update(@path id: NizeApi.UUID, @body body: UpdateConversationRequest):
    | ConversationSummary
    | NizeApi.NotFoundError
    | NizeApi.ValidationError
    | NizeApi.UnauthorizedError;

  /**
   * Delete a conversation and all its messages.
   */
  @delete
  @route("/{id}")
  @summary("Delete conversation")
  delete(@path id: NizeApi.UUID): {
    @statusCode statusCode: 204;
  } | NizeApi.NotFoundError | NizeApi.UnauthorizedError;

  /**
   * Bulk save messages for a conversation.
   * Replaces existing messages with the provided set.
   * Called by the chat backend after stream completion.
   */
  @put
  @route("/{id}/messages")
  @summary("Save conversation messages")
  saveMessages(@path id: NizeApi.UUID, @body body: BulkSaveMessagesRequest):
    | {
        @statusCode statusCode: 204;
      }
    | NizeApi.NotFoundError
    | NizeApi.ValidationError
    | NizeApi.UnauthorizedError;
}
