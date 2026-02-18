// @zen-component: PLAN-027-Compaction

import { CompactMessage, CompactState, ContextSummary } from "./types";

// Default size limit for tool output summarization (100KB)
const DEFAULT_RESPONSE_SIZE_LIMIT = 100 * 1024;

// =============================================================================
// Tool Output Summarizer
// =============================================================================

export interface ToolOutputSummarizer {
  summarizeLargeOutputs(messages: readonly CompactMessage[], limit?: number): CompactMessage[];
}

export class DefaultToolOutputSummarizer implements ToolOutputSummarizer {
  private readonly defaultLimit: number;

  constructor(defaultLimit: number = DEFAULT_RESPONSE_SIZE_LIMIT) {
    this.defaultLimit = defaultLimit;
  }

  summarizeLargeOutputs(messages: readonly CompactMessage[], limit?: number): CompactMessage[] {
    const effectiveLimit = limit ?? this.defaultLimit;

    return messages.map((message) => {
      // Only process assistant messages (which may contain tool outputs)
      if (message.role !== "assistant") {
        return message;
      }

      // Check if content exceeds limit
      const content = message.content;
      if (content.length <= effectiveLimit) {
        return message;
      }

      // Summarize large content
      const summarized = this.summarizeContent(content, effectiveLimit);
      return {
        ...message,
        content: summarized,
      };
    });
  }

  private summarizeContent(content: string, limit: number): string {
    // Try to intelligently truncate
    // If it looks like JSON, try to summarize the structure
    if (content.trim().startsWith("{") || content.trim().startsWith("[")) {
      try {
        const parsed = JSON.parse(content);
        const summary = this.summarizeJSON(parsed, limit);
        return summary;
      } catch {
        // Not valid JSON, fall through to simple truncation
      }
    }

    // Simple truncation with ellipsis
    const previewLength = Math.min(limit * 0.8, 1000);
    return `${content.slice(0, previewLength)}...\n\n[Content truncated from ${formatSize(content.length)} to fit context limit]`;
  }

  private summarizeJSON(value: unknown, limit: number): string {
    if (Array.isArray(value)) {
      const itemCount = value.length;
      // Show first few items
      const preview = value.slice(0, 3);
      const previewStr = JSON.stringify(preview, null, 2);
      if (itemCount > 3) {
        return `[Array with ${itemCount} items]\nFirst 3 items:\n${previewStr}\n\n... and ${itemCount - 3} more items`;
      }
      return previewStr;
    }

    if (typeof value === "object" && value !== null) {
      const keys = Object.keys(value);
      const keyCount = keys.length;
      // Show structure with truncated values
      const summary: Record<string, unknown> = {};
      let currentSize = 0;
      const sizeLimit = limit * 0.8;

      for (const key of keys.slice(0, 10)) {
        const val = (value as Record<string, unknown>)[key];
        const valStr = JSON.stringify(val);

        if (currentSize + valStr.length > sizeLimit) {
          summary[key] = `[${typeof val}: ${valStr.length} bytes]`;
        } else {
          summary[key] = val;
          currentSize += valStr.length;
        }
      }

      if (keyCount > 10) {
        summary["..."] = `${keyCount - 10} more keys`;
      }

      return JSON.stringify(summary, null, 2);
    }

    // Primitive value
    const str = String(value);
    if (str.length > limit) {
      return str.slice(0, limit) + "...";
    }
    return str;
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// =============================================================================
// Compaction
// =============================================================================

/**
 * Conditionally compact a message history.
 *
 * When message count exceeds `maxMessages`:
 * 1. Summarize large tool outputs
 * 2. Generate a text summary of older messages
 * 3. Prepend summary as a system message
 * 4. Keep only the last `maxMessages` messages
 *
 * When count is under the threshold, returns messages unchanged.
 */
export function maybeCompact(messages: readonly CompactMessage[], maxMessages: number, summarizer: ToolOutputSummarizer = new DefaultToolOutputSummarizer()): CompactState {
  if (messages.length <= maxMessages) {
    return { messages: [...messages] };
  }

  // Summarize large tool outputs first
  const processed = summarizer.summarizeLargeOutputs(messages);

  // Generate summary from older messages
  const olderMessages = processed.slice(0, processed.length - maxMessages);
  const recentMessages = processed.slice(-maxMessages);

  const summaryText = olderMessages.map((m) => `${m.role}: ${m.content}`).join("\n");

  const summary: ContextSummary = { text: summaryText };

  // Prepend summary as system message
  const compactedMessages: CompactMessage[] = [
    {
      role: "system",
      content: `[Context from earlier in the conversation]\n${summaryText}`,
    },
    ...recentMessages,
  ];

  return {
    messages: compactedMessages,
    summary,
  };
}
