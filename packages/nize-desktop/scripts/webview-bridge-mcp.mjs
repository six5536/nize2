// @zen-component: PLAN-015-McpServer
//
// WebView Bridge MCP Server — exposes MCP tools over stdio
// that translate to WebSocket commands sent to the bridge client injected
// in the Tauri webview.
//
// Usage:
//   node webview-bridge-mcp.mjs [--ws-port=19570]
//
// Debug-only — not included in production builds.

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { WebSocketServer, WebSocket } from "ws";
import { parseArgs } from "node:util";
import { randomUUID } from "node:crypto";
import { z } from "zod";

const { values: args } = parseArgs({
  options: {
    "ws-port": { type: "string", default: "19570" },
  },
});

const WS_PORT = parseInt(args["ws-port"], 10);

// ---------------------------------------------------------------------------
// WebSocket server — bridge client connections
// ---------------------------------------------------------------------------

/** @type {Set<WebSocket>} */
const clients = new Set();

/** @type {Map<string, { resolve: (v: unknown) => void, reject: (e: Error) => void }>} */
const pendingRequests = new Map();

const wss = new WebSocketServer({ port: WS_PORT });

wss.on("connection", (ws) => {
  clients.add(ws);
  // Use stderr for logging — stdout is reserved for MCP stdio protocol.
  process.stderr.write(`[webview-bridge-mcp] bridge client connected (${clients.size} total)\n`);

  ws.on("message", (data) => {
    try {
      const msg = JSON.parse(data.toString());
      const pending = pendingRequests.get(msg.id);
      if (pending) {
        pendingRequests.delete(msg.id);
        if (msg.error) {
          pending.reject(new Error(msg.error));
        } else {
          pending.resolve(msg.result);
        }
      }
    } catch (e) {
      process.stderr.write(`[webview-bridge-mcp] failed to parse message: ${e}\n`);
    }
  });

  ws.on("close", () => {
    clients.delete(ws);
    process.stderr.write(`[webview-bridge-mcp] bridge client disconnected (${clients.size} total)\n`);
  });
});

/**
 * Send a command to all connected bridge clients and collect the first response.
 * @param {string} method
 * @param {Record<string, unknown>} [params]
 * @returns {Promise<unknown>}
 */
function sendCommand(method, params) {
  return new Promise((resolve, reject) => {
    if (clients.size === 0) {
      reject(new Error("No bridge client connected. Is the Tauri webview running?"));
      return;
    }

    const id = randomUUID();
    const timeout = setTimeout(() => {
      pendingRequests.delete(id);
      reject(new Error(`Bridge command '${method}' timed out after 30s`));
    }, 30_000);

    pendingRequests.set(id, {
      resolve: (v) => {
        clearTimeout(timeout);
        resolve(v);
      },
      reject: (e) => {
        clearTimeout(timeout);
        reject(e);
      },
    });

    const msg = JSON.stringify({ id, method, params });
    // Send to first connected client
    const client = clients.values().next().value;
    client.send(msg);
  });
}

// ---------------------------------------------------------------------------
// MCP server
// ---------------------------------------------------------------------------

const mcpServer = new McpServer(
  {
    name: "webview-bridge",
    version: "0.1.0",
  },
  {
    capabilities: {
      tools: {},
    },
  },
);

// @zen-impl: PLAN-015-2.2

mcpServer.tool("webview_snapshot", "Returns an accessibility-tree-style snapshot of the current DOM (parent + iframe). " + "Each interactive element has a ref id for targeting with other tools.", {}, async () => {
  const result = await sendCommand("snapshot");
  return { content: [{ type: "text", text: String(result) }] };
});

mcpServer.tool("webview_evaluate", "Execute arbitrary JavaScript in the webview context and return the result.", { expression: z.string().describe("JavaScript expression to evaluate") }, async ({ expression }) => {
  const result = await sendCommand("evaluate", { expression });
  return {
    content: [{ type: "text", text: typeof result === "string" ? result : JSON.stringify(result, null, 2) }],
  };
});

mcpServer.tool(
  "webview_click",
  "Click an element identified by CSS selector, text content, or ref id from a snapshot.",
  {
    selector: z.string().optional().describe("CSS selector to find the element"),
    ref: z.number().optional().describe("Ref id from a previous snapshot"),
    text: z.string().optional().describe("Exact text content of the element"),
  },
  async (params) => {
    await sendCommand("click", params);
    return { content: [{ type: "text", text: "Clicked." }] };
  },
);

mcpServer.tool(
  "webview_fill",
  "Fill a form field (input/textarea) with the given value.",
  {
    selector: z.string().optional().describe("CSS selector to find the element"),
    ref: z.number().optional().describe("Ref id from a previous snapshot"),
    value: z.string().describe("Value to fill into the field"),
  },
  async (params) => {
    await sendCommand("fill", params);
    return { content: [{ type: "text", text: "Filled." }] };
  },
);

mcpServer.tool(
  "webview_select_option",
  "Select an option in a <select> element.",
  {
    selector: z.string().optional().describe("CSS selector to find the select element"),
    ref: z.number().optional().describe("Ref id from a previous snapshot"),
    value: z.string().describe("Value of the option to select"),
  },
  async (params) => {
    await sendCommand("select_option", params);
    return { content: [{ type: "text", text: "Selected." }] };
  },
);

mcpServer.tool(
  "webview_navigate",
  "Navigate the iframe (or parent) to a URL.",
  {
    url: z.string().describe("URL to navigate to"),
    target: z.string().optional().describe('Target frame: "parent" or "iframe" (default: "iframe")'),
  },
  async (params) => {
    await sendCommand("navigate", params);
    return { content: [{ type: "text", text: "Navigated." }] };
  },
);

mcpServer.tool(
  "webview_console",
  "Return recent console log/error/warn messages from the webview.",
  {
    limit: z.number().optional().describe("Maximum number of entries to return (default: 100)"),
  },
  async (params) => {
    const result = await sendCommand("console", params);
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  },
);

mcpServer.tool("webview_screenshot", "Take a screenshot of the webview and return base64-encoded PNG. " + "Requires html2canvas to be loaded in the webview.", {}, async () => {
  const result = await sendCommand("screenshot");
  if (typeof result === "string") {
    return {
      content: [{ type: "image", data: result, mimeType: "image/png" }],
    };
  }
  return { content: [{ type: "text", text: "Screenshot failed: no data returned" }] };
});

// ---------------------------------------------------------------------------
// Stdio transport — the AI agent spawns this process and communicates via
// stdin/stdout.  The WebSocket server for the bridge client runs in parallel.
// ---------------------------------------------------------------------------

const transport = new StdioServerTransport();
await mcpServer.connect(transport);

process.stderr.write(`[webview-bridge-mcp] MCP server running (stdio), WebSocket on ws://127.0.0.1:${WS_PORT}\n`);
