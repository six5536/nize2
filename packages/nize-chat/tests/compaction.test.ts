import { describe, it, expect } from "vitest";
import { DefaultToolOutputSummarizer, maybeCompact } from "../src/compaction.js";
import type { CompactMessage } from "../src/types.js";

// @zen-test: PLAN-027-Compaction
describe("maybeCompact", () => {
  it("should not compact when message count is under threshold", () => {
    const messages: CompactMessage[] = [
      { role: "user", content: "hello" },
      { role: "assistant", content: "hi" },
    ];

    const result = maybeCompact(messages, 100);
    expect(result.summary).toBeUndefined();
    expect(result.messages).toEqual(messages);
  });

  it("should compact when message count exceeds threshold", () => {
    const messages: CompactMessage[] = Array.from({ length: 50 }, (_, i) => ({
      role: (i % 2 === 0 ? "user" : "assistant") as "user" | "assistant",
      content: `message ${i}`,
    }));

    const result = maybeCompact(messages, 10);
    expect(result.summary).toBeDefined();
    // recent messages + 1 system summary message
    expect(result.messages.length).toBe(11);
  });

  it("should prepend summary as system message when compacting", () => {
    const messages: CompactMessage[] = Array.from({ length: 20 }, (_, i) => ({
      role: (i % 2 === 0 ? "user" : "assistant") as "user" | "assistant",
      content: `message ${i}`,
    }));

    const result = maybeCompact(messages, 5);
    expect(result.messages[0].role).toBe("system");
    expect(result.messages[0].content).toContain("[Context from earlier in the conversation]");
  });

  it("should preserve recent messages after compaction", () => {
    const messages: CompactMessage[] = [
      { role: "user", content: "old message 1" },
      { role: "assistant", content: "old response 1" },
      { role: "user", content: "old message 2" },
      { role: "assistant", content: "old response 2" },
      { role: "user", content: "recent message" },
      { role: "assistant", content: "recent response" },
    ];

    const result = maybeCompact(messages, 2);
    // system summary + 2 recent
    expect(result.messages.length).toBe(3);
    expect(result.messages[1].content).toBe("recent message");
    expect(result.messages[2].content).toBe("recent response");
  });

  it("should include older messages in summary text", () => {
    const messages: CompactMessage[] = [
      { role: "user", content: "first question" },
      { role: "assistant", content: "first answer" },
      { role: "user", content: "second question" },
      { role: "assistant", content: "second answer" },
    ];

    const result = maybeCompact(messages, 2);
    expect(result.summary).toBeDefined();
    expect(result.summary!.text).toContain("first question");
    expect(result.summary!.text).toContain("first answer");
  });
});

// @zen-test: PLAN-027-ToolOutputSummarizer
describe("ToolOutputSummarizer", () => {
  describe("summarize large outputs", () => {
    it("should not modify messages under the limit", () => {
      const summarizer = new DefaultToolOutputSummarizer(1000);
      const messages: CompactMessage[] = [
        { role: "user", content: "What's in that file?" },
        { role: "assistant", content: "Small response" },
      ];

      const result = summarizer.summarizeLargeOutputs(messages);
      expect(result).toEqual(messages);
    });

    it("should truncate assistant messages exceeding the limit", () => {
      const summarizer = new DefaultToolOutputSummarizer(100);
      const largeContent = "x".repeat(500);
      const messages: CompactMessage[] = [
        { role: "user", content: "What's in that directory?" },
        { role: "assistant", content: largeContent },
      ];

      const result = summarizer.summarizeLargeOutputs(messages);

      expect(result[0]).toEqual(messages[0]); // User message unchanged
      expect(result[1].content.length).toBeLessThan(largeContent.length);
      expect(result[1].content).toContain("truncated");
    });

    it("should not modify user messages even if large", () => {
      const summarizer = new DefaultToolOutputSummarizer(100);
      const largeUserContent = "y".repeat(500);
      const messages: CompactMessage[] = [{ role: "user", content: largeUserContent }];

      const result = summarizer.summarizeLargeOutputs(messages);
      expect(result[0].content).toBe(largeUserContent);
    });
  });

  describe("JSON summarization", () => {
    it("should summarize JSON arrays with item count", () => {
      const summarizer = new DefaultToolOutputSummarizer(200);
      const largeArray = JSON.stringify(
        Array.from({ length: 100 }, (_, i) => ({
          id: i,
          name: `Item ${i}`,
        })),
      );
      const messages: CompactMessage[] = [{ role: "assistant", content: largeArray }];

      const result = summarizer.summarizeLargeOutputs(messages);

      expect(result[0].content).toContain("Array with 100 items");
      expect(result[0].content).toContain("First 3 items");
      expect(result[0].content.length).toBeLessThan(largeArray.length);
    });

    it("should summarize JSON objects with truncated values", () => {
      const summarizer = new DefaultToolOutputSummarizer(500);
      const largeObject = {
        key1: "short",
        key2: "a".repeat(1000),
        key3: "also short",
      };
      const messages: CompactMessage[] = [{ role: "assistant", content: JSON.stringify(largeObject) }];

      const result = summarizer.summarizeLargeOutputs(messages);

      expect(result[0].content).toContain("key1");
      expect(result[0].content).toContain("key2");
      expect(result[0].content.length).toBeLessThan(JSON.stringify(largeObject).length);
    });

    it("should handle nested JSON objects", () => {
      const summarizer = new DefaultToolOutputSummarizer(300);
      const nested = {
        level1: {
          level2: {
            data: "x".repeat(1000),
          },
        },
      };
      const messages: CompactMessage[] = [{ role: "assistant", content: JSON.stringify(nested) }];

      const result = summarizer.summarizeLargeOutputs(messages);

      expect(result[0].content).toContain("level1");
      expect(result[0].content.length).toBeLessThan(JSON.stringify(nested).length);
    });
  });

  describe("integration with maybeCompact", () => {
    it("should summarize large outputs during compaction", () => {
      const largeToolOutput = JSON.stringify({
        files: Array.from({ length: 50 }, (_, i) => `file${i}.txt`),
      });
      const messages: CompactMessage[] = [
        { role: "user", content: "List files" },
        { role: "assistant", content: largeToolOutput },
        { role: "user", content: "Thanks" },
        { role: "assistant", content: "You're welcome" },
        { role: "user", content: "Another question" },
        { role: "assistant", content: "Another answer" },
      ];

      // Use small limit so tool output gets summarized
      const summarizer = new DefaultToolOutputSummarizer(200);
      const result = maybeCompact(messages, 2, summarizer);

      expect(result.summary).toBeDefined();
      expect(result.messages.length).toBe(3); // system + 2 recent
    });
  });
});
