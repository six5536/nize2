# PLAN-015: WebView Bridge MCP

| Field              | Value                                              |
|--------------------|----------------------------------------------------|
| **Status**         | not-started                                        |
| **Workflow**       | lateral                                            |
| **Reference**      | PLAN-014 (nize-web dev hot reload)                 |
| **Traceability**   | —                                                  |

## Goal

Enable AI agents (and automated tools) to inspect and interact with the live Tauri webview — including the desktop shell and the embedded nize-web iframe — via an MCP server. This bridges the gap caused by macOS WKWebView not exposing CDP.

## Architecture

```
AI Agent
    │
    ▼  (MCP protocol — streamable HTTP or stdio)
WebView Bridge MCP Server  (Node.js, local, debug-only)
    │
    ▼  (WebSocket)
Bridge client script (injected into Tauri webview at startup)
    │
    ├──► Desktop shell DOM  (localhost:1420)
    └──► nize-web iframe DOM  (/nize-web/ — same origin)
```

Because the desktop shell and nize-web now share the same origin (PLAN-014 same-origin proxy), a single injected script can traverse both the parent document and the iframe's `contentDocument`.

## MCP Tools

| Tool | Description |
|------|-------------|
| `webview_snapshot` | Returns an accessibility-tree-style snapshot of the current DOM (parent + iframe). Similar to Playwright's `page.snapshot()`. |
| `webview_evaluate` | Execute arbitrary JS in the webview context and return the result. |
| `webview_click` | Click an element identified by CSS selector, text content, or ref id. |
| `webview_fill` | Fill a form field (input/textarea) by selector. |
| `webview_select_option` | Select an option in a `<select>` element. |
| `webview_navigate` | Navigate the iframe (or parent) to a URL. |
| `webview_console` | Return recent console log/error/warn messages. |
| `webview_screenshot` | Take a screenshot via `html2canvas` or similar and return base64 PNG. |

## Components

### C1 — Bridge Client (`packages/nize-desktop/src/webview-bridge.ts`)

Injected into the Tauri webview via a `<script>` tag (dev only).

- Opens a WebSocket to `ws://127.0.0.1:<bridge-port>`
- Listens for JSON commands: `{ id, method, params }`
- Executes against the live DOM (parent and iframe)
- Sends JSON responses: `{ id, result }` or `{ id, error }`
- Captures `console.*` calls via monkey-patching and buffers recent entries
- Generates accessibility-tree snapshots (role, name, value, ref) by walking the DOM

### C2 — MCP Server (`packages/nize-desktop/scripts/webview-bridge-mcp.mjs`)

A standalone Node.js process exposing MCP tools over streamable HTTP.

- Accepts a single WebSocket connection from the bridge client
- Exposes MCP tools that translate to bridge commands
- Waits for bridge client connection before reporting tools as available
- Debug-only — not included in production builds

### C3 — Tauri Integration (`crates/app/nize_desktop/src/lib.rs`)

- In debug builds, inject the bridge client script into the webview via `WebviewWindow::eval()` in the `setup` hook (or add a `<script>` tag to the Vite dev server)
- The bridge MCP server is started as a sidecar (or manually by the developer)

## Decisions

### D1 — Bridge Protocol

Use WebSocket (not Tauri IPC) because:
- The MCP server runs as a separate process, not inside the Tauri app
- WebSocket is bidirectional and works with standard Node.js
- No Tauri plugin dependency

### D2 — Snapshot Format

Use a simplified accessibility tree (YAML-like, matching Playwright's snapshot format) so AI agents can parse it consistently. Each interactive element gets a `ref` id for targeting.

### D3 — Debug-Only

The bridge client and MCP server are never included in production builds. The script injection is gated on `#[cfg(debug_assertions)]`.

### D4 — Single vs Dual Injection

With same-origin proxy (PLAN-014), a single bridge client in the parent can access `iframe.contentDocument` directly. No need for dual injection.

## Steps

### Phase 1 — Bridge Client

- [ ] **1.1** Create `packages/nize-desktop/src/webview-bridge.ts`:
  - WebSocket client connecting to `ws://127.0.0.1:19570` (configurable)
  - Command dispatcher: `snapshot`, `evaluate`, `click`, `fill`, `navigate`, `console`
  - DOM snapshot walker: produces `{ role, name, value, ref }` tree
  - Console interceptor: monkey-patch `console.log/warn/error`, buffer last 100 entries
  - Auto-reconnect on disconnect
- [ ] **1.2** Add snapshot logic:
  - Walk `document.body` recursively
  - For each element: compute ARIA role (or infer from tag), accessible name, value
  - Assign sequential `ref` ids to interactive elements
  - Traverse into same-origin iframes via `contentDocument`
- [ ] **1.3** Add event reporting:
  - `navigate` events (pushState, popState, iframe load)
  - `console` entries with level, message, timestamp

### Phase 2 — MCP Server

- [ ] **2.1** Create `packages/nize-desktop/scripts/webview-bridge-mcp.mjs`:
  - WebSocket server on port 19570
  - MCP server (streamable HTTP) on port 19571
  - Map MCP tool calls → WebSocket commands → responses
  - Handle connection lifecycle (wait for client, reconnect)
- [ ] **2.2** Define MCP tool schemas:
  - `webview_snapshot`: no params → returns YAML string
  - `webview_evaluate`: `{ expression: string }` → returns JSON result
  - `webview_click`: `{ selector?: string, ref?: string, text?: string }` → void
  - `webview_fill`: `{ selector: string, value: string }` → void
  - `webview_navigate`: `{ url: string, target?: "parent" | "iframe" }` → void
  - `webview_console`: `{ limit?: number }` → returns array of entries
- [ ] **2.3** Add to MCP config for VS Code / Claude Desktop so agents can discover it

### Phase 3 — Injection

- [ ] **3.1** Option A: Add `<script src="/webview-bridge.js">` via Vite plugin in dev
- [ ] **3.2** Option B: Inject via `WebviewWindow::eval()` in Rust setup hook
- [ ] **3.3** Choose one, verify bridge connects on `cargo tauri dev` startup

### Phase 4 — Testing

- [ ] **4.1** Manual test: start `cargo tauri dev`, verify bridge MCP tools work from an AI agent
- [ ] **4.2** Verify: `webview_snapshot` returns the full tree including nize-web iframe content
- [ ] **4.3** Verify: `webview_click` and `webview_fill` interact correctly with form elements

## Risks

| Risk | Mitigation |
|------|------------|
| DOM snapshot too large for AI context | Limit depth, skip hidden elements, truncate text |
| Console monkey-patching breaks existing logging | Preserve original functions, only intercept in bridge |
| WebSocket port conflicts | Use configurable port, default 19570 |
| iframe `contentDocument` access blocked despite same-origin | Verify PLAN-014 same-origin proxy is working before bridge |
| Bridge script interferes with app behavior | Minimal footprint — no DOM mutations, read-only except for click/fill commands |

## Resolved Questions

1. **Auto-start**: The bridge MCP server is auto-started by `cargo tauri dev` (dev mode only). No manual step required.
2. **Screenshot approach**: Use `html2canvas` (dev dependency only — not included in production builds).
3. **Multiple agents**: Yes — support multiple simultaneous agent connections to the bridge MCP server.
