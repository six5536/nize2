// @zen-test: PLAN-029-3.2
// @zen-test: PLAN-029-3.3
// @zen-test: PLAN-029-3.4

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

describe("mcp-client: createMcpSession", () => {
  const originalFetch = globalThis.fetch;

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it("should call POST /auth/mcp-tokens with overwrite=true", async () => {
    const mockFetch = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify({ token: "test-token-abc", id: "tok-1", name: "nize-desktop-chat", createdAt: "2026-01-01" }), { status: 201 }));
    globalThis.fetch = mockFetch;

    // Mock createMCPClient — we can't fully test MCP connection without a server
    vi.mock("@ai-sdk/mcp", () => ({
      createMCPClient: vi.fn().mockResolvedValue({
        tools: vi.fn().mockResolvedValue({}),
        close: vi.fn().mockResolvedValue(undefined),
      }),
    }));

    const { createMcpSession } = await import("../src/mcp-client.js");
    await createMcpSession("http://127.0.0.1:3001", "session=abc", "http://127.0.0.1:19560");

    // Verify token creation call
    expect(mockFetch).toHaveBeenCalledWith(
      "http://127.0.0.1:3001/api/auth/mcp-tokens",
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          "Content-Type": "application/json",
          cookie: "session=abc",
        }),
        body: JSON.stringify({ name: "nize-desktop-chat", overwrite: true }),
      }),
    );
  });

  it("should throw on token creation failure", async () => {
    globalThis.fetch = vi.fn().mockResolvedValueOnce(new Response("Unauthorized", { status: 401 }));

    // Re-import to get fresh module
    vi.resetModules();
    vi.mock("@ai-sdk/mcp", () => ({
      createMCPClient: vi.fn(),
    }));

    const { createMcpSession } = await import("../src/mcp-client.js");

    await expect(createMcpSession("http://127.0.0.1:3001", "session=abc", "http://127.0.0.1:19560")).rejects.toThrow("Failed to create MCP token: 401");
  });
});

describe("types: ChatConfig defaults", () => {
  it("should have tool calling fields in DEFAULT_CHAT_CONFIG", async () => {
    const { DEFAULT_CHAT_CONFIG } = await import("../src/types.js");

    expect(DEFAULT_CHAT_CONFIG.toolsEnabled).toBe(true);
    expect(DEFAULT_CHAT_CONFIG.toolsMaxSteps).toBe(10);
    expect(DEFAULT_CHAT_CONFIG.toolsSystemPrompt).toContain("discover_tools");
    expect(DEFAULT_CHAT_CONFIG.toolsSystemPrompt).toContain("execute_tool");
  });
});

describe("chat-config: fetchChatConfig reads tool settings", () => {
  const originalFetch = globalThis.fetch;

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it("should parse tool config from API response", async () => {
    const mockConfigResponse = {
      items: [
        { key: "agent.model.name", value: null, default_value: "anthropic:claude-haiku-4-5-20251001" },
        { key: "agent.model.temperature", value: null, default_value: "0.7" },
        { key: "agent.compaction.maxMessages", value: null, default_value: "20" },
        { key: "agent.tools.enabled", value: "false", default_value: "true" },
        { key: "agent.tools.maxSteps", value: "5", default_value: "10" },
        { key: "agent.tools.systemPrompt", value: "Custom prompt", default_value: "Default prompt" },
      ],
    };
    globalThis.fetch = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify(mockConfigResponse)));

    const { fetchChatConfig } = await import("../src/chat-config.js");
    const config = await fetchChatConfig("http://127.0.0.1:3001", "session=abc");

    expect(config.toolsEnabled).toBe(false);
    expect(config.toolsMaxSteps).toBe(5);
    expect(config.toolsSystemPrompt).toBe("Custom prompt");
  });

  it("should use defaults when tool config not present", async () => {
    const mockConfigResponse = {
      items: [
        { key: "agent.model.name", value: null, default_value: "anthropic:claude-haiku-4-5-20251001" },
        { key: "agent.model.temperature", value: null, default_value: "0.7" },
        { key: "agent.compaction.maxMessages", value: null, default_value: "20" },
      ],
    };
    globalThis.fetch = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify(mockConfigResponse)));

    const { fetchChatConfig } = await import("../src/chat-config.js");
    const config = await fetchChatConfig("http://127.0.0.1:3001", "session=abc");

    expect(config.toolsEnabled).toBe(true);
    expect(config.toolsMaxSteps).toBe(10);
    expect(config.toolsSystemPrompt).toContain("discover_tools");
  });
});
