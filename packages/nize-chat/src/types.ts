// @zen-component: PLAN-027-Types

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
// @zen-impl: PLAN-028-3.2
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
}

/** Default chat configuration values (must match migration 0003_config.sql) */
export const DEFAULT_CHAT_CONFIG: ChatConfig = {
  modelName: "anthropic:claude-haiku-4-5-20251001",
  temperature: 0.7,
  compactionMaxMessages: 20,
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
