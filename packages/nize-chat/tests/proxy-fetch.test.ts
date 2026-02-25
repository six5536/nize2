import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createProxyFetch } from "../src/proxy-fetch.js";

// @awa-test: PLAN-028-3.1

describe("createProxyFetch", () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    globalThis.fetch = vi.fn().mockResolvedValue(new Response("ok"));
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it("should rewrite URL to proxy endpoint with target and provider params", async () => {
    const proxyFetch = createProxyFetch("http://127.0.0.1:3001", "session=abc", "anthropic");

    await proxyFetch("https://api.anthropic.com/v1/messages", { method: "POST" });

    expect(globalThis.fetch).toHaveBeenCalledWith("http://127.0.0.1:3001/api/ai-proxy?target=https%3A%2F%2Fapi.anthropic.com%2Fv1%2Fmessages&provider=anthropic", expect.objectContaining({ method: "POST" }));
  });

  it("should forward cookie header", async () => {
    const proxyFetch = createProxyFetch("http://127.0.0.1:3001", "session=abc", "openai");

    await proxyFetch("https://api.openai.com/v1/chat/completions", {
      method: "POST",
      headers: { "content-type": "application/json" },
    });

    const callArgs = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
    const headers = callArgs[1].headers as Headers;
    expect(headers.get("cookie")).toBe("session=abc");
  });

  it("should remove auth headers", async () => {
    const proxyFetch = createProxyFetch("http://127.0.0.1:3001", "session=abc", "anthropic");

    await proxyFetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      headers: {
        authorization: "Bearer sk-secret",
        "x-api-key": "sk-ant-secret",
        "x-goog-api-key": "goog-secret",
      },
    });

    const callArgs = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
    const headers = callArgs[1].headers as Headers;
    expect(headers.get("authorization")).toBeNull();
    expect(headers.get("x-api-key")).toBeNull();
    expect(headers.get("x-goog-api-key")).toBeNull();
  });

  it("should encode provider type in URL", async () => {
    const proxyFetch = createProxyFetch("http://127.0.0.1:3001", "session=abc", "google");

    await proxyFetch("https://generativelanguage.googleapis.com/v1/models/gemini-2.0-flash", {});

    expect(globalThis.fetch).toHaveBeenCalledWith(expect.stringContaining("provider=google"), expect.anything());
  });

  it("should default method to POST", async () => {
    const proxyFetch = createProxyFetch("http://127.0.0.1:3001", "session=abc", "anthropic");

    await proxyFetch("https://api.anthropic.com/v1/messages");

    const callArgs = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(callArgs[1].method).toBe("POST");
  });

  it("should handle URL object input", async () => {
    const proxyFetch = createProxyFetch("http://127.0.0.1:3001", "session=abc", "openai");

    await proxyFetch(new URL("https://api.openai.com/v1/chat/completions"), { method: "POST" });

    expect(globalThis.fetch).toHaveBeenCalledWith(expect.stringContaining("target=https%3A%2F%2Fapi.openai.com%2Fv1%2Fchat%2Fcompletions"), expect.anything());
  });
});
