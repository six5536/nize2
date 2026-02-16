# PLAN-021: Next.js as Tauri Frontend (HMR Fix)

**Status:** complete
**Workflow direction:** lateral
**Traceability:** PLAN-012 (nize-web sidecar), PLAN-014 (nize-web dev hot-reload), PLAN-015 (webview bridge)

## Problem

The current architecture loads nize-web (Next.js) inside an iframe within the Vite/React desktop shell. This requires:

1. A Vite dev proxy (`/nize-web → 127.0.0.1:3100`) for same-origin in dev
2. A custom nize-web-server.mjs proxy that re-proxies to an internal Next.js port
3. WebSocket upgrade proxying for HMR through two layers of proxy

This triple-proxy chain (Tauri webview → Vite → nize-web-server → Next.js) breaks Next.js HMR reliably (see console errors: `SyntaxError: The string did not match the expected pattern` in sidebar.tsx and page.tsx, plus 404s for `__nextjs_original-stack-frames` and `__nextjs_devtools_config`).

## Proposal

Make nize-web (Next.js) the **primary frontend** loaded directly by Tauri's webview, eliminating the Vite/React shell and all proxy layers entirely.

### Key Question: Can Next.js pages access Tauri Rust commands?

**Yes.** Tauri 2 injects its IPC bridge (`window.__TAURI_INTERNALS__`) into whatever URL the webview loads — it doesn't care whether the page comes from Vite, Next.js, or a static file. The `@tauri-apps/api` npm package works from any page loaded in the webview, as long as:

1. The page is loaded via `devUrl` (dev) or `frontendDist` (production)
2. The relevant Tauri plugins are registered in the Rust side
3. CSP allows inline scripts (already `null` in tauri.conf.json)

Since nize-web already uses only the REST API (no Tauri calls), adding `@tauri-apps/api` as an optional dependency is straightforward. Desktop-specific code can be gated on `window.__TAURI_INTERNALS__` existence.

## Architecture Changes

### What Gets Removed

- Vite/React shell framework: App.tsx, AuthGate.tsx, TabView.tsx, main.tsx, index.html, vite.config.ts, auth/*
- Vite dev server and all its dependencies
- nize-web-server.mjs proxy layer (the reverse proxy between the front port and internal Next.js port)
- iframe embedding of nize-web
- `NizeWebSidecar` struct and spawn logic in lib.rs (now `#[cfg(not(debug_assertions))]`)

### What Gets Ported to nize-web (step 12)

- MainApp.tsx (Hello test button) → nize-web settings/desktop page, gated behind `isTauri()`
- McpClientSettings.tsx, McpClientCard.tsx, McpTokenSection.tsx → nize-web settings/desktop page, gated behind `isTauri()`
- UpdateChecker.tsx → nize-web component, gated behind `isTauri()`
- webview-bridge.ts → nize-web layout.tsx injection (dev-only)

### What Gets Added / Changed

- `tauri.conf.json`: `devUrl` → `http://localhost:3100/` (Next.js dev server), `frontendDist` → Next.js standalone output directory
- nize-web gains `@tauri-apps/api` as an optional dependency
- nize-web gains a Tauri-aware API discovery mechanism (try `invoke("get_api_port")`, fallback to `__NIZE_ENV__`)
- Desktop-only UI (MCP client settings, update checker) moves into nize-web pages gated by `isTauri()` check
- Webview bridge client moves into nize-web (dev-only injection via Next.js instrumentation or script tag)

### What Stays the Same

- nize-web-server.mjs remains for **production sidecar mode** (serving standalone Next.js + `__nize-env.js` injection), but loses the proxy layer — it becomes a thin wrapper that just serves the standalone build
- nize-web's auth system (cookie-based, REST API) — unchanged
- All Rust sidecar management (PGlite, nize_desktop_server, nize_terminator) — unchanged
- Webview bridge MCP server architecture — unchanged (WebSocket to injected client)
- nize-web continues to work as a standalone web app (cloud deployment)

## Detailed Steps

### Phase 1: Next.js as devUrl target

1. **Update `tauri.conf.json`**
   - `devUrl` → `http://localhost:3100`
   - `frontendDist` → path to Next.js standalone output (resolved later)
   - `beforeDevCommand` → remove Vite, add `bun run --cwd packages/nize-web dev` (or keep the sidecar approach but simpler)

2. **Update Next.js dev config**
   - Remove `basePath` entirely (was only needed for iframe sub-path; not needed for cloud either)
   - Remove `NIZE_WEB_BASE_PATH` env var usage from next.config.ts and nize-web-server.mjs
   - Ensure Next.js dev server binds to `localhost:3100`

3. **Move `__nize-env.js` injection to Next.js itself**
   - In dev: add a Next.js middleware or `next.config.ts` rewrites to serve `/__nize-env.js`
   - In production sidecar: nize-web-server.mjs continues to serve it

4. **Test HMR works directly**
   - `cargo tauri dev` should open the Tauri webview pointing at `http://localhost:3100`
   - Editing nize-web pages should trigger instant HMR with no proxy layers

### Phase 2: Tauri API Integration in nize-web

5. **Add `@tauri-apps/api` to nize-web**
   - Add as an optional/dev dependency
   - Create `lib/tauri.ts` utility:
     ```typescript
     export function isTauri(): boolean {
       return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
     }
     ```

6. **Replace `__nize-env.js` API port discovery with Tauri IPC (when available)**
   - Update `lib/api.ts`: try `invoke("get_api_port")` when `isTauri()`, fallback to `__NIZE_ENV__`
   - This eliminates the need for `__nize-env.js` in desktop dev mode entirely

7. **Gate desktop-only features**
   - Create a `components/desktop/` directory for Tauri-only UI
   - Move MCP client settings, update checker from nize-desktop into nize-web under desktop gate
   - Use dynamic imports: `if (isTauri()) { const { McpSettings } = await import(...) }`

### Phase 3: Simplify Sidecar Infrastructure

8. **Simplify nize-web-server.mjs**
   - In **dev mode**: no longer needed — Tauri loads Next.js directly via `devUrl`
   - In **production**: serves standalone Next.js build + `__nize-env.js`, but no proxy to API (API is on a separate port, nize-web uses `apiUrl()` which resolves via Tauri IPC or env)
   - Remove the reverse-proxy and WebSocket proxy code

9. **Update Rust sidecar management (`lib.rs`)**
   - **Dev**: don't spawn nize-web sidecar (Next.js is started by `beforeDevCommand`)
   - **Production**: spawn nize-web as before but with simplified server script
   - Keep `get_nize_web_port` (needed for production loading page), wrap in conditional body (`#[cfg(not(debug_assertions))]` for real impl, returns error in debug)
   - Keep `get_api_port`, `get_mcp_port`

10. **Move webview bridge injection to nize-web**
    - Add bridge client script to nize-web's layout.tsx (dev-only, behind `process.env.NODE_ENV === "development"` and `isTauri()`)
    - Or use Next.js `instrumentation.ts` / script injection

### Phase 4: Cleanup

11. **Remove `packages/nize-desktop/src/`**
    - Delete App.tsx, AuthGate.tsx, MainApp.tsx, TabView.tsx, auth/*, settings/*
    - Delete index.html, vite.config.ts, Vite/React dependencies from package.json
    - Keep `packages/nize-desktop/` as a **scripts-only package** (build tooling)
    - Keep `packages/nize-desktop/scripts/webview-bridge-mcp.mjs` (MCP server for VS Code)
    - Keep build scripts: build-pglite-server.mjs, build-mcp-remote.mjs, build-nize-web.mjs
    - Keep pglite-server.mjs (PGlite sidecar)

12. **Port desktop UI to nize-web settings**
    - Move MCP client settings (McpClientSettings.tsx, McpClientCard.tsx, McpTokenSection.tsx) to `packages/nize-web/app/settings/desktop/`
    - Move UpdateChecker.tsx to `packages/nize-web/components/desktop/`
    - All ported code must be gated behind `isTauri()` (hidden when running as web app)
    - These components call Tauri Rust commands (`invoke()`), so import `@tauri-apps/api` dynamically

13. **Update ARCHITECTURE.md**
    - Desktop shell is now Next.js (not React + Vite)
    - Remove references to iframe, Vite proxy, TabView
    - Update component diagram

14. **Update `.github/instructions/tauri.instructions.md`**
    - Reflect new architecture

### Phase 5: Production Build

15. **Configure `frontendDist` for production**
    - Next.js standalone output → `packages/nize-web/.next/standalone`
    - Or use the nize-web-server.mjs sidecar to serve the standalone build (Tauri points webview at `http://localhost:<port>`)
    - **Decision needed**: static export vs sidecar-served. Sidecar-served is simpler since Next.js standalone mode already works.

16. **Update `beforeBuildCommand`**
    - Build nize-web (`next build`), then reference the output in `frontendDist`
    - Or keep sidecar approach: `frontendDist` is a minimal HTML that waits for sidecar, then redirects

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Next.js SSR pages incompatible with Tauri webview | Medium | nize-web already uses `"use client"` throughout; no SSR issues expected |
| `@tauri-apps/api` causes issues when nize-web runs outside Tauri | Low | Guard all Tauri calls behind `isTauri()` check; tree-shake in web builds |
| Production build: Tauri can't serve Next.js standalone directly via `frontendDist` (needs a running server) | Medium | Keep sidecar for production: Tauri spawns nize-web-server, then opens webview to `http://localhost:<port>`. Use a small loading page as `frontendDist` that polls until sidecar is ready |
| Loss of desktop-shell-only features (TabView, settings) | Low | Port progressively to nize-web; desktop settings page gated on `isTauri()` |
| Webview bridge MCP stops working | Low | Port bridge client injection to nize-web layout; no architectural change |
| basePath removal breaks cloud deployment | None | basePath was only needed for iframe sub-path; cloud deployment uses no basePath |

## Production Architecture Decision

**Option A — Sidecar-served (recommended):**
Tauri spawns nize-web standalone server on an ephemeral port (as today), then opens the webview to `http://localhost:<port>`. This is the simplest path — nize-web-server.mjs already does this.

- `frontendDist`: small HTML that shows "Loading..." and polls for the nize-web sidecar port via `invoke("get_nize_web_port")`, then navigates to it.
- Pro: Next.js server features (API routes, middleware) work in production
- Con: still need to spawn a sidecar process

**Option B — Static export:**
Configure Next.js with `output: "export"` for desktop builds. Tauri serves static files directly.

- `frontendDist`: `packages/nize-web/out/`
- Pro: no sidecar needed, faster startup
- Con: loses Next.js server features (middleware, ISR, API routes); may need significant refactoring if server features are used later

**Recommendation:** Option A. The sidecar overhead is minimal, and it preserves full Next.js capabilities. The dev experience improves dramatically (direct devUrl, native HMR), which is the primary goal.

## Completion Criteria

- [x] `cargo tauri dev` opens nize-web directly in the Tauri webview (no iframe)
- [x] Next.js HMR works instantly without errors
- [x] Auth flow works end-to-end in the webview
- [x] Desktop-specific features (MCP client settings, update checker) visible only in Tauri
- [x] nize-web deploys as a standalone web app without Tauri dependencies
- [x] Webview bridge MCP tools work as before
- [x] Production build works (sidecar-served)

## Decisions (Resolved)

1. **Cloud deployment basePath**: No basePath needed. It was only required for the iframe proxy sub-path (`/nize-web/`). Both desktop and cloud deployments use root path.

2. **Vite package fate**: Keep `packages/nize-desktop/` as a **scripts-only package** (build tooling for pglite-server, mcp-remote, nize-web production build, webview-bridge MCP server). All UI code (MCP client settings, update checker) moves to nize-web settings pages, gated behind `isTauri()` since it calls through to Rust.

3. **`beforeDevCommand` sequencing**: `beforeDevCommand` builds Rust sidecar binaries, sets up sidecar binaries, builds pglite-server, builds mcp-remote, then starts Next.js dev server. Rust `run()` does **not** spawn nize-web in dev — Tauri connects via `devUrl`. In production, Rust spawns nize-web sidecar as before.

4. **API proxy in nize-web dev**: Use Next.js `rewrites` in `next.config.ts` to proxy `/auth/*`, `/config/*`, `/admin/*`, `/api/*` to `http://127.0.0.1:3001` in dev. In Tauri desktop, use `invoke("get_api_port")` via IPC for port discovery. In cloud, use `NEXT_PUBLIC_API_URL` or relative URLs.
