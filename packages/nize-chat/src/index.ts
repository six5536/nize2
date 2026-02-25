// @awa-component: PLAN-027-Barrel

export { chatApp } from "./app";
export { processChat, ConversationNotFoundError } from "./chat-service";
export type { ProcessChatResult } from "./chat-service";
export { fetchChatConfig } from "./chat-config";
export { createMcpSession } from "./mcp-client";
export { maybeCompact, DefaultToolOutputSummarizer } from "./compaction";
export type { ToolOutputSummarizer } from "./compaction";
export { getChatModel, getProviderFromSpec } from "./model-registry";
export type { GetChatModelOptions } from "./model-registry";
export { createProxyFetch } from "./proxy-fetch";
export type { ChatRequest, ChatConfig, CompactMessage, CompactState, ContextSummary } from "./types";
export { DEFAULT_CHAT_CONFIG, DEFAULT_TOOLS_SYSTEM_PROMPT } from "./types";
