// @zen-component: CHAT-MessageBubble
// Type definitions and guards for UIMessage parts

import type { UIMessage } from "ai";

/**
 * Tool invocation states from AI SDK v6
 * @zen-impl: CHAT-7.2_AC-1
 */
export type ToolState = "input-streaming" | "input-available" | "output-available" | "output-error";

/**
 * Text part of a message
 */
export interface TextPart {
  type: "text";
  text: string;
}

/**
 * Reasoning part (thinking/chain-of-thought)
 */
export interface ReasoningPart {
  type: "reasoning";
  reasoning: string;
}

/**
 * Tool invocation part with state machine
 * Matches AI SDK v6 tool parts - can be "dynamic-tool" or "tool-{toolName}"
 */
export interface ToolInvocationPart {
  type: string; // "dynamic-tool" or "tool-{toolName}" pattern
  toolCallId: string;
  toolName?: string;
  input: unknown;
  state: ToolState;
  output?: unknown;
  errorText?: string;
}

/**
 * Source reference part
 */
export interface SourcePart {
  type: "source";
  source: {
    type: string;
    id: string;
  };
}

/**
 * File attachment part
 */
export interface FilePart {
  type: "file";
  mimeType: string;
  data: string;
}

export type MessagePart = TextPart | ReasoningPart | ToolInvocationPart | SourcePart | FilePart;

/**
 * Type guard for text parts
 */
export function isTextPart(part: unknown): part is TextPart {
  return typeof part === "object" && part !== null && "type" in part && part.type === "text" && "text" in part;
}

/**
 * Type guard for reasoning parts
 */
export function isReasoningPart(part: unknown): part is ReasoningPart {
  return typeof part === "object" && part !== null && "type" in part && part.type === "reasoning" && "reasoning" in part;
}

/**
 * Type guard for tool invocation parts
 * AI SDK v6 uses type "tool-{toolName}" pattern (e.g., "tool-execute_tool", "tool-discover_tools")
 */
export function isToolInvocationPart(part: unknown): part is ToolInvocationPart {
  if (typeof part !== "object" || part === null || !("type" in part) || !("state" in part)) {
    return false;
  }
  const type = (part as { type: unknown }).type;
  // Match "tool-*" pattern (e.g., "tool-execute_tool", "tool-discover_tools")
  // and "dynamic-tool" (MCP tools via @ai-sdk/mcp)
  return typeof type === "string" && (type.startsWith("tool-") || type === "dynamic-tool");
}

/**
 * Type guard for source parts
 */
export function isSourcePart(part: unknown): part is SourcePart {
  return typeof part === "object" && part !== null && "type" in part && part.type === "source";
}

/**
 * Type guard for file parts
 */
export function isFilePart(part: unknown): part is FilePart {
  return typeof part === "object" && part !== null && "type" in part && part.type === "file";
}

/**
 * Extract all text content from a message
 * Useful for copy functionality
 */
export function extractTextContent(message: UIMessage): string {
  if (!message.parts || message.parts.length === 0) {
    // Fallback to legacy content field
    return (message as unknown as { content?: string }).content ?? "";
  }

  return message.parts
    .filter(isTextPart)
    .map((part) => part.text)
    .join("\n");
}

/**
 * Check if a tool is in a loading state
 */
export function isToolLoading(state: ToolState): boolean {
  return state === "input-streaming" || state === "input-available";
}

/**
 * Check if a tool has completed with output
 */
export function isToolComplete(state: ToolState): boolean {
  return state === "output-available" || state === "output-error";
}

/**
 * Check if a tool has errored
 */
export function isToolError(state: ToolState): boolean {
  return state === "output-error";
}
