import { describe, it, expect } from "vitest";
import { getChatModel, getProviderFromSpec } from "../src/model-registry.js";

// @awa-test: PLAN-028-3.4

describe("getChatModel", () => {
  it("should throw on invalid spec without colon", () => {
    expect(() => getChatModel("invalid")).toThrow("Invalid model format");
  });

  it("should throw on unsupported provider", () => {
    expect(() => getChatModel("azure:gpt-4")).toThrow("Unsupported model provider: azure");
  });

  it("should resolve anthropic model", () => {
    const model = getChatModel("anthropic:claude-haiku-4-5-20251001");
    expect(model.modelId).toBe("claude-haiku-4-5-20251001");
  });

  it("should resolve openai model", () => {
    const model = getChatModel("openai:gpt-4o-mini");
    expect(model.modelId).toBe("gpt-4o-mini");
  });

  it("should resolve google model", () => {
    const model = getChatModel("google:gemini-2.0-flash");
    expect(model.modelId).toBe("gemini-2.0-flash");
  });

  it("should accept custom fetch option", () => {
    const customFetch = async () => new Response("ok");
    // Should not throw
    const model = getChatModel("anthropic:claude-haiku-4-5-20251001", { fetch: customFetch });
    expect(model.modelId).toBe("claude-haiku-4-5-20251001");
  });

  it("should accept custom baseUrls option", () => {
    const model = getChatModel("openai:gpt-4o-mini", {
      baseUrls: { openai: "https://my-proxy.example.com/v1" },
    });
    expect(model.modelId).toBe("gpt-4o-mini");
  });
});

describe("getProviderFromSpec", () => {
  it("should extract anthropic from spec", () => {
    expect(getProviderFromSpec("anthropic:claude-haiku-4-5-20251001")).toBe("anthropic");
  });

  it("should extract openai from spec", () => {
    expect(getProviderFromSpec("openai:gpt-4o-mini")).toBe("openai");
  });

  it("should throw on invalid spec without colon", () => {
    expect(() => getProviderFromSpec("nocolon")).toThrow("Invalid model format");
  });
});
