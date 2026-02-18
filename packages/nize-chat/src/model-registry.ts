// @zen-component: PLAN-027-ModelRegistry

import { createAnthropic } from "@ai-sdk/anthropic";
import { createOpenAI } from "@ai-sdk/openai";
import { createGoogleGenerativeAI } from "@ai-sdk/google";

// Placeholder API key passed to provider constructors so the SDK's client-side
// validation passes. The real key is injected server-side by the Rust AI proxy
// (POST /ai-proxy) which strips this placeholder and adds the decrypted key.
const PROXY_PLACEHOLDER_KEY = "proxy-managed";

/** Options for configuring the model provider */
export interface GetChatModelOptions {
  /** Custom fetch function (e.g. proxy fetch) */
  fetch?: typeof globalThis.fetch;
  /** Custom base URLs per provider */
  baseUrls?: {
    anthropic?: string;
    openai?: string;
    google?: string;
  };
}

/**
 * Resolve an AI SDK model instance from a `provider:model` spec string.
 *
 * Supported providers: anthropic, openai, google.
 *
 * @param spec - Model spec, e.g. "anthropic:claude-haiku-4-5-20251001"
 * @param options - Optional custom fetch and base URLs
 * @returns AI SDK LanguageModel instance
 */
// @zen-impl: PLAN-028-3.4
export function getChatModel(spec: string, options?: GetChatModelOptions) {
  const colonIdx = spec.indexOf(":");
  if (colonIdx === -1) {
    throw new Error(`Invalid model format: ${spec}. Expected "provider:model"`);
  }

  const provider = spec.slice(0, colonIdx);
  const modelName = spec.slice(colonIdx + 1);

  switch (provider) {
    case "anthropic": {
      const anthropic = createAnthropic({
        apiKey: PROXY_PLACEHOLDER_KEY,
        ...(options?.baseUrls?.anthropic ? { baseURL: options.baseUrls.anthropic } : {}),
        ...(options?.fetch ? { fetch: options.fetch } : {}),
      });
      return anthropic(modelName);
    }
    case "openai": {
      const openai = createOpenAI({
        apiKey: PROXY_PLACEHOLDER_KEY,
        ...(options?.baseUrls?.openai ? { baseURL: options.baseUrls.openai } : {}),
        ...(options?.fetch ? { fetch: options.fetch } : {}),
      });
      return openai(modelName);
    }
    case "google": {
      const google = createGoogleGenerativeAI({
        apiKey: PROXY_PLACEHOLDER_KEY,
        ...(options?.baseUrls?.google ? { baseURL: options.baseUrls.google } : {}),
        ...(options?.fetch ? { fetch: options.fetch } : {}),
      });
      return google(modelName);
    }
    default:
      throw new Error(`Unsupported model provider: ${provider}`);
  }
}

/**
 * Extract the provider name from a model spec string.
 *
 * @param spec - Model spec, e.g. "anthropic:claude-haiku-4-5-20251001"
 * @returns Provider name, e.g. "anthropic"
 */
export function getProviderFromSpec(spec: string): string {
  const colonIdx = spec.indexOf(":");
  if (colonIdx === -1) {
    throw new Error(`Invalid model format: ${spec}. Expected "provider:model"`);
  }
  return spec.slice(0, colonIdx);
}
