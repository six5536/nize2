---
applyTo: "packages/nize-desktop/**"
---

# Tauri Desktop Development

## Running the Desktop App

Start the Tauri dev environment with:

```sh
cargo tauri dev
```

This starts Vite (port 1420), builds and runs the Rust backend (nize_desktop), the
API sidecar (nize_desktop_server), PGlite, and the nize-web Next.js sidecar.

## Debugging the WebView via MCP

The `nize-webview-bridge` MCP server (defined in `.vscode/mcp.json`) exposes tools
for inspecting and interacting with the live Tauri webview. It only works during
`cargo tauri dev` — the bridge client is injected by a Vite plugin in dev mode only.

### Available Tools

| Tool                    | Purpose                                         |
| ----------------------- | ----------------------------------------------- |
| `webview_snapshot`      | DOM accessibility-tree snapshot with ref ids    |
| `webview_click`         | Click an element by ref, selector, or text      |
| `webview_fill`          | Fill an input/textarea                          |
| `webview_select_option` | Select from a `<select>` dropdown               |
| `webview_evaluate`      | Run arbitrary JS in the webview                 |
| `webview_navigate`      | Navigate the iframe or parent frame             |
| `webview_console`       | Retrieve recent console log/error/warn messages |
| `webview_screenshot`    | Capture a PNG screenshot of the webview         |

### Typical Workflow

1. Ensure `cargo tauri dev` is running and the app window is visible.
2. Use `webview_snapshot` to see the current DOM state and ref ids.
3. Use `webview_click`, `webview_fill`, etc. to interact with elements by ref id.
4. Use `webview_screenshot` to visually confirm the result.
5. Use `webview_console` to check for errors.

### Architecture

- **Bridge client** (`src/webview-bridge.ts`): Injected into the webview via Vite
  plugin. Connects to the MCP server over WebSocket (port 19570).
- **MCP server** (`scripts/webview-bridge-mcp.mjs`): Stdio MCP server that VS Code
  spawns automatically. Translates MCP tool calls into WebSocket commands.
