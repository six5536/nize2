// @awa-component: PLAN-028-ProxyFetch

/**
 * Create a custom `fetch` function that routes requests through the Rust AI proxy.
 *
 * The proxy injects provider-specific auth headers server-side, so decrypted
 * API keys never leave the Rust process.
 *
 * @param apiBaseUrl - Base URL of the Rust API (e.g. "http://127.0.0.1:3001")
 * @param cookie - Cookie header to forward for auth
 * @param providerType - Provider type: "anthropic", "openai", or "google"
 * @returns A `fetch`-compatible function for use with AI SDK provider constructors
 */
// @awa-impl: PLAN-028-3.1
export function createProxyFetch(apiBaseUrl: string, cookie: string, providerType: string): typeof globalThis.fetch {
  return async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    // Extract the original target URL
    const originalUrl = typeof input === "string" ? input : input instanceof URL ? input.toString() : (input as Request).url;

    // Build the proxy URL with query params
    const proxyUrl = `${apiBaseUrl}/api/ai-proxy?target=${encodeURIComponent(originalUrl)}&provider=${encodeURIComponent(providerType)}`;

    // Forward init options, replacing the URL and adding the cookie
    const headers = new Headers(init?.headers);
    headers.set("cookie", cookie);
    // Remove any auth headers — the proxy injects them
    headers.delete("authorization");
    headers.delete("x-api-key");
    headers.delete("x-goog-api-key");

    return globalThis.fetch(proxyUrl, {
      ...init,
      method: init?.method ?? "POST",
      headers,
    });
  };
}
