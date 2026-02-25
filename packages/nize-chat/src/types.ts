// @awa-component: PLAN-027-Types

import type { UIMessage } from "ai";

// ============================================================================
// Chat Request/Response
// ============================================================================

/** Incoming chat request from the AI SDK frontend */
export interface ChatRequest {
  /** Messages array (AI SDK UIMessage format) */
  messages: UIMessage[];
  /** Conversation ID (optional, creates new if not provided) */
  conversationId?: string;
}

// ============================================================================
// Chat Config
// ============================================================================

/** Chat configuration fetched from the Rust API */
// @awa-impl: PLAN-028-3.2
export interface ChatConfig {
  /** Model spec in provider:model format (e.g. "anthropic:claude-haiku-4-5-20251001") */
  modelName: string;
  /** Temperature for LLM generation (0–2) */
  temperature: number;
  /** Max messages before compaction triggers */
  compactionMaxMessages: number;
  /** Custom base URLs per provider (from agent.baseUrl.* config) */
  baseUrls?: {
    anthropic?: string;
    openai?: string;
    google?: string;
  };
  // @awa-impl: PLAN-029-3.3
  /** Whether MCP tool calling is enabled */
  toolsEnabled: boolean;
  /** Maximum number of tool-call steps per message */
  toolsMaxSteps: number;
  /** System prompt to prepend when tools are enabled */
  toolsSystemPrompt: string;
}

/** Default system prompt for MCP tools guidance */
export const DEFAULT_TOOLS_SYSTEM_PROMPT = "You have access to tools for discovering and executing external MCP tools. " + "Use `discover_tools` to find relevant tools, `get_tool_schema` to understand parameters, " + "and `execute_tool` to run them. Use `list_tool_domains` and `browse_tool_domain` to explore available categories.";

/** Default chat configuration values (must match migration 0003_config.sql) */
export const DEFAULT_CHAT_CONFIG: ChatConfig = {
  modelName: "anthropic:claude-haiku-4-5-20251001",
  temperature: 0.7,
  compactionMaxMessages: 20,
  toolsEnabled: true,
  toolsMaxSteps: 10,
  toolsSystemPrompt: DEFAULT_TOOLS_SYSTEM_PROMPT,
};

// ============================================================================
// Compaction
// ============================================================================

/** Simplified message for compaction (flattened from UIMessage) */
export interface CompactMessage {
  role: "user" | "assistant" | "system";
  content: string;
}

/** Optional context summary from compaction */
export interface ContextSummary {
  /** Text summary of compacted older messages */
  text: string;
}

/** State after compaction processing */
export interface CompactState {
  /** Messages to send to the LLM (possibly trimmed) */
  messages: CompactMessage[];
  /** Summary of older messages (if compaction triggered) */
  summary?: ContextSummary;
}
