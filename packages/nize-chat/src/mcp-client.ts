// @zen-component: PLAN-029-McpClient

import { createMCPClient, type MCPTransport } from "@ai-sdk/mcp";

/** Token name used for nize-chat MCP sessions. Overwritten on each request. */
const MCP_TOKEN_NAME = "nize-desktop-chat";

/** Timeout for MCP client connection + initialization (ms). */
const MCP_CONNECT_TIMEOUT_MS = 10_000;

/** MCP protocol version (must match rmcp's LATEST_PROTOCOL_VERSION). */
const MCP_PROTOCOL_VERSION = "2025-03-26";

// ============================================================================
// Custom Streamable HTTP Transport
// ============================================================================

/**
 * Minimal Streamable HTTP transport for MCP.
 *
 * This replaces @ai-sdk/mcp's built-in HTTP transport because that transport
 * only processes SSE events with `event: "message"`, but rmcp (and the MCP
 * spec's default) sends events without an explicit `event:` field.
 * `eventsource-parser` returns `undefined` for the event type in that case,
 * so all responses are silently dropped and `createMCPClient` hangs.
 *
 * This transport processes ALL SSE data events containing valid JSON-RPC
 * messages, regardless of the event type field.
 */
class StreamableHttpTransport implements MCPTransport {
  private url: string;
  private headers: Record<string, string>;
  private sessionId?: string;
  private abortController?: AbortController;

  onclose?: () => void;
  onerror?: (error: Error) => void;
  onmessage?: (message: unknown) => void;

  constructor(url: string, headers: Record<string, string>) {
    this.url = url;
    this.headers = headers;
  }

  async start(): Promise<void> {
    this.abortController = new AbortController();
  }

  async send(message: Record<string, unknown>): Promise<void> {
    const headers: Record<string, string> = {
      ...this.headers,
      "Content-Type": "application/json",
      Accept: "application/json, text/event-stream",
      "mcp-protocol-version": MCP_PROTOCOL_VERSION,
    };
    if (this.sessionId) {
      headers["mcp-session-id"] = this.sessionId;
    }

    const response = await fetch(this.url, {
      method: "POST",
      headers,
      body: JSON.stringify(message),
      signal: this.abortController?.signal,
    });

    const sid = response.headers.get("mcp-session-id");
    if (sid) this.sessionId = sid;

    if (response.status === 202) return;

    if (!response.ok) {
      const text = await response.text().catch(() => "");
      throw new Error(`MCP HTTP ${response.status}: ${text}`);
    }

    // Notifications don't expect a response
    if (!("id" in message)) return;

    const contentType = response.headers.get("content-type") || "";

    if (contentType.includes("application/json")) {
      const data = await response.json();
      const msgs = Array.isArray(data) ? data : [data];
      for (const m of msgs) this.onmessage?.(m);
      return;
    }

    if (contentType.includes("text/event-stream") && response.body) {
      this.consumeSseStream(response.body);
      return;
    }
  }

  async close(): Promise<void> {
    if (this.sessionId && this.abortController && !this.abortController.signal.aborted) {
      await fetch(this.url, {
        method: "DELETE",
        headers: { ...this.headers, "mcp-session-id": this.sessionId },
        signal: this.abortController.signal,
      }).catch(() => {});
    }
    this.abortController?.abort();
    this.onclose?.();
  }

  /**
   * Read an SSE stream and deliver JSON-RPC messages via onmessage.
   *
   * Unlike @ai-sdk/mcp's built-in transport, this does NOT filter by
   * event type — it processes all events that contain valid JSON data.
   * This is required for rmcp compatibility where SSE events omit the
   * `event: message` field (relying on the SSE spec default).
   */
  private consumeSseStream(body: ReadableStream<Uint8Array>): void {
    const reader = body.pipeThrough(new TextDecoderStream() as unknown as TransformStream<Uint8Array, string>).getReader();
    let buffer = "";

    const processChunks = async () => {
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          // Normalize CRLF to LF
          buffer += value.replace(/\r\n/g, "\n");

          // Split on double-newline event boundaries
          let boundary = buffer.indexOf("\n\n");
          while (boundary !== -1) {
            const raw = buffer.slice(0, boundary);
            buffer = buffer.slice(boundary + 2);

            // Extract data lines (may be multi-line per SSE spec)
            const dataLines: string[] = [];
            for (const line of raw.split("\n")) {
              if (line.startsWith("data:")) {
                const payload = line.slice(5);
                dataLines.push(payload.startsWith(" ") ? payload.slice(1) : payload);
              }
              // Ignore comment lines (":"), id, event, retry fields
            }

            const data = dataLines.join("\n");
            if (!data) {
              boundary = buffer.indexOf("\n\n");
              continue;
            }

            try {
              const parsed = JSON.parse(data);
              this.onmessage?.(parsed);
            } catch {
              // Non-JSON data (e.g. priming events with empty data)
            }

            boundary = buffer.indexOf("\n\n");
          }
        }
      } catch (err) {
        if (err instanceof Error && err.name === "AbortError") return;
        this.onerror?.(err instanceof Error ? err : new Error(String(err)));
      }
    };

    processChunks();
  }
}

// ============================================================================
// MCP Session
// ============================================================================

/**
 * Create an MCP client session by obtaining a bearer token from the REST API
 * and connecting to the MCP server via Streamable HTTP.
 *
 * The token is created with `overwrite: true`, so any previous
 * `nize-desktop-chat` token for the user is atomically revoked.
 *
 * @param apiBaseUrl - Base URL of the Rust API (e.g. "http://127.0.0.1:3001")
 * @param cookie - Cookie header for JWT auth against the REST API
 * @param mcpBaseUrl - Base URL of the MCP server (e.g. "http://127.0.0.1:19560")
 * @returns MCPClient instance (caller must close when done)
 */
// @zen-impl: PLAN-029-3.2
export async function createMcpSession(apiBaseUrl: string, cookie: string, mcpBaseUrl: string) {
  // Create/overwrite MCP bearer token via REST API
  const tokenRes = await fetch(`${apiBaseUrl}/api/auth/mcp-tokens`, {
    method: "POST",
    headers: { "Content-Type": "application/json", cookie },
    body: JSON.stringify({ name: MCP_TOKEN_NAME, overwrite: true }),
  });

  if (!tokenRes.ok) {
    const errBody = await tokenRes.text().catch(() => "unknown error");
    throw new Error(`Failed to create MCP token: ${tokenRes.status} ${errBody}`);
  }

  const tokenData = (await tokenRes.json()) as { token: string };
  const bearerToken = tokenData.token;

  console.log("[mcp] Token obtained, connecting to MCP server...");

  // Connect to MCP server using custom Streamable HTTP transport.
  // The built-in @ai-sdk/mcp HTTP transport silently drops SSE events
  // without `event: message` — rmcp omits this field, causing a hang.
  const transport = new StreamableHttpTransport(`${mcpBaseUrl}/mcp`, {
    Authorization: `Bearer ${bearerToken}`,
  });

  const mcpClient = await Promise.race([createMCPClient({ transport }), new Promise<never>((_, reject) => setTimeout(() => reject(new Error("MCP client connection timed out")), MCP_CONNECT_TIMEOUT_MS))]);

  console.log("[mcp] MCP client connected (initialize done)");

  return mcpClient;
}
