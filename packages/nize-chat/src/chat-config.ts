// @awa-component: PLAN-027-ChatConfig

import { ChatConfig, DEFAULT_CHAT_CONFIG, DEFAULT_TOOLS_SYSTEM_PROMPT } from "./types";

/**
 * Fetch chat configuration from the Rust API.
 *
 * Reads agent.model.name, agent.model.temperature,
 * agent.compaction.maxMessages, and agent.baseUrl.* from the config endpoint.
 *
 * @param apiBaseUrl - Base URL of the Rust API (e.g. "http://127.0.0.1:3001")
 * @param cookie - Cookie header to forward for auth
 * @returns Resolved ChatConfig
 */
// @awa-impl: PLAN-028-3.3
export async function fetchChatConfig(apiBaseUrl: string, cookie: string): Promise<ChatConfig> {
  try {
    const res = await fetch(`${apiBaseUrl}/api/config/user`, {
      headers: { cookie },
    });
    if (!res.ok) {
      console.error(`Failed to fetch chat config: ${res.status}`);
      return { ...DEFAULT_CHAT_CONFIG };
    }

    const data = (await res.json()) as {
      items?: Array<{ key: string; value: string | null; default_value: string; defaultValue?: string }>;
    };

    const items = data.items ?? [];
    const get = (key: string, fallback: string): string => {
      const item = items.find((i) => i.key === key);
      return item?.value ?? item?.defaultValue ?? item?.default_value ?? fallback;
    };

    // Read base URLs — empty string means use default (provider SDK default)
    const anthropicBaseUrl = get("agent.baseUrl.anthropic", "");
    const openaiBaseUrl = get("agent.baseUrl.openai", "");
    const googleBaseUrl = get("agent.baseUrl.google", "");

    const baseUrls: ChatConfig["baseUrls"] = {};
    if (anthropicBaseUrl) baseUrls.anthropic = anthropicBaseUrl;
    if (openaiBaseUrl) baseUrls.openai = openaiBaseUrl;
    if (googleBaseUrl) baseUrls.google = googleBaseUrl;

    return {
      modelName: get("agent.model.name", DEFAULT_CHAT_CONFIG.modelName),
      temperature: parseFloat(get("agent.model.temperature", String(DEFAULT_CHAT_CONFIG.temperature))),
      compactionMaxMessages: parseInt(get("agent.compaction.maxMessages", String(DEFAULT_CHAT_CONFIG.compactionMaxMessages)), 10),
      baseUrls: Object.keys(baseUrls).length > 0 ? baseUrls : undefined,
      // @awa-impl: PLAN-029-3.4 — read tool calling config
      toolsEnabled: get("agent.tools.enabled", String(DEFAULT_CHAT_CONFIG.toolsEnabled)) === "true",
      toolsMaxSteps: parseInt(get("agent.tools.maxSteps", String(DEFAULT_CHAT_CONFIG.toolsMaxSteps)), 10),
      toolsSystemPrompt: get("agent.tools.systemPrompt", DEFAULT_TOOLS_SYSTEM_PROMPT),
    };
  } catch (error) {
    console.error("Error fetching chat config, using defaults:", error);
    return { ...DEFAULT_CHAT_CONFIG };
  }
}
