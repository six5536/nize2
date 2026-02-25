/**
 * TypeSpec definitions for Chat Trace API.
 * Dev panel visibility for debugging AI conversations.
 *
 * Ported from ref project: submodules/nize/packages/api-types/src/trace.tsp
 */
import "@typespec/http";
import "@typespec/rest";
import "./API-NIZE-common.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;

namespace NizeApi;

// ============================================================================
// Trace Models
// ============================================================================

enum TraceEventType {
  conversationStart: "conversation_start",
  promptConstruction: "prompt_construction",
  ragSearch: "rag_search",
  compaction: "compaction",
  toolCall: "tool_call",
  modelConfig: "model_config",
  decision: "decision",
  phaseTiming: "phase_timing",
  streamComplete: "stream_complete",
}

model TraceEvent {
  type: TraceEventType;
  timestamp: string;
  messageId: string;
}

model ChatTrace {
  id: UUID;
  conversationId: UUID;
  messageId: string;
  events: TraceEvent[];
  createdAt: string;
  expiresAt: string;
}

model ChatTraceResponse extends ChatTrace {}

// ============================================================================
// Routes
// ============================================================================

@route("/dev")
@tag("Development")
interface DevTraceRoutes {
  /**
   * Returns trace events for a conversation.
   * Admin-only: returns 404 for non-admin users.
   */
  @route("/chat_trace")
  @get
  @summary("Get chat trace")
  getChatTrace(
    @query conversationId: UUID,
  ): ChatTraceResponse | NotFoundError | UnauthorizedError;
}
