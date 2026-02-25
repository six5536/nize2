# PLAN-012: nize-web Sidecar and Tab UI

| Field              | Value                                              |
|--------------------|----------------------------------------------------|
| **Status**         | in-progress                                        |
| **Workflow**       | top-down                                           |
| **Reference**      | PLAN-007 (pglite-migration), PLAN-010 (cloud-server-split) |
| **Traceability**   | —                                                  |

## Goal

Add a new **nize-web** sidecar — a Next.js React application (hello world) — launched by nize_desktop via the bundled Node.js binary. Convert the nize-desktop main view into a **tab UI** with:

1. **Tab 1 ("Desktop")**: existing nize-desktop content (`MainApp`)
2. **Tab 2 ("Web")**: loads nize-web in a scrollable iframe

## Architecture

### Process Model (updated)

```
Tauri (nize_desktop)
  ├── spawns: node pglite-server.mjs …
  ├── spawns: nize_desktop_server …
  ├── spawns: node .next/standalone/server.js --port 0   ← NEW (nize-web)
  │            ↳ prints {"port": P} to stdout (via custom wrapper)
  │            ↳ Next.js on localhost:P
  └── spawns: nize_terminator …
```

### Bundle Layout (macOS .app, additions only)

```
Nize Desktop.app/Contents/
└── Resources/
    └── nize-web/          ← NEW
        ├── server.mjs     ← wrapper entry point (reads --port, prints JSON)
        └── standalone/    ← Next.js standalone output
```

### Data Flow

```
AuthGate → MainApp (existing)
         ↓
         TabView
           ├── Tab "Desktop" → existing MainApp content
           └── Tab "Web"     → <iframe src="http://127.0.0.1:{nize_web_port}" />
```

## Decisions

### D1 — Next.js Standalone Output

Next.js `output: "standalone"` produces a self-contained `server.js` that can be run with `node server.js`. This avoids shipping `node_modules`. The standalone output + static assets are bundled as Tauri resources (same pattern as pglite).

### D2 — Sidecar Protocol

Reuse the existing JSON-on-stdout sidecar protocol: the wrapper script starts the Next.js server on an ephemeral port and prints `{"port": N}` to stdout once listening. The Rust side reads this and stores the port.

### D3 — Tab UI in React

Generic, reusable n-tab component in the existing React app. No additional UI library needed. The `TabView` accepts an array of tab definitions (label + content renderer) so additional tabs can be added later without refactoring. Tabs are styled buttons toggling visibility of tab content panels.

### D4 — iframe for nize-web

The Web tab renders an `<iframe>` pointing to `http://127.0.0.1:{port}`. The iframe is styled to fill the tab content area and scroll independently. Tauri CSP must allow framing localhost.

### D5 — Dev Mode

In dev mode, nize-web runs via `next dev` on an ephemeral port (port 0). The sidecar wrapper detects the actual bound port and reports it via the JSON protocol. Same approach as production — no fixed port fallback needed.

## Steps

### Phase 1 — Create nize-web Next.js Package

Set up the Next.js application as a new package under `packages/nize-web`.

- [ ] **1.1** Create `packages/nize-web/` with `npx create-next-app@16` (App Router, TypeScript, no Tailwind, no ESLint — keep minimal)
- [ ] **1.2** Implement hello world page: `app/page.tsx` renders a centered `<h1>Hello from nize-web</h1>`
- [ ] **1.3** Configure `next.config.ts`: set `output: "standalone"`, set default port via env var
- [ ] **1.4** Add `"dev"` and `"build"` scripts in `package.json`
- [ ] **1.5** Verify: `npm run dev` serves the page, `npm run build` produces `.next/standalone/server.js`

### Phase 2 — Sidecar Wrapper Script

Create a Node.js wrapper that starts the Next.js standalone server on a dynamic port and reports readiness via the JSON sidecar protocol.

- [ ] **2.1** Create `packages/nize-web/scripts/nize-web-server.mjs`:
  - Accept `--port=<N>` (0 = ephemeral)
  - Set `PORT` env var, spawn `.next/standalone/server.js` (or run inline)
  - Detect when the server is listening (poll `http://127.0.0.1:<port>`)
  - Print `{"port": N}` to stdout
  - Forward SIGTERM/SIGINT for graceful shutdown
- [ ] **2.2** Add esbuild/bundling script `packages/nize-web/scripts/build-nize-web-server.mjs`:
  - Copy `.next/standalone/` and `.next/static/` into `crates/app/nize_desktop/resources/nize-web/`
  - Copy `nize-web-server.mjs` wrapper into same location
- [ ] **2.3** Verify: `node resources/nize-web/nize-web-server.mjs --port=0` prints `{"port": N}` and serves the page

### Phase 3 — Rust: Spawn nize-web Sidecar

Integrate nize-web into the nize_desktop launch sequence.

- [ ] **3.1** Add `NizeWebSidecar` struct to `lib.rs` (or new `nize_web.rs` module):
  - Holds `Child` process and `port: u16`
- [ ] **3.2** Add `start_nize_web_sidecar(node_bin, server_script)` function:
  - Resolve `nize-web-server.mjs` from resources (same pattern as pglite)
  - Spawn `node nize-web-server.mjs --port=0`
  - Read `{"port": N}` from stdout
  - Return `NizeWebSidecar`
- [ ] **3.3** Update `AppServices` to include `nize_web: Option<NizeWebSidecar>`
- [ ] **3.4** Call `start_nize_web_sidecar()` after API sidecar in `run()`:
  - Append kill command to terminator manifest
  - Log the bound port
- [ ] **3.5** Add Tauri command `get_nize_web_port` (same pattern as `get_api_port`)
- [ ] **3.6** Register the new command in `run_tauri()` invoke_handler
- [ ] **3.7** On exit, drop nize_web sidecar (kill child process)

### Phase 4 — Tauri Configuration

Update Tauri config to bundle nize-web resources and allow iframe embedding.

- [ ] **4.1** Add `resources/nize-web/*` → `nize-web/` in `tauri.conf.json` bundle resources
- [ ] **4.2** Update `beforeDevCommand` to also build nize-web:
  - Add `npm run --prefix packages/nize-web build` and the copy/bundle step
- [ ] **4.3** Update `beforeBuildCommand` similarly for production builds
- [ ] **4.4** Ensure CSP allows `frame-src http://127.0.0.1:*` (currently CSP is `null`, so no issue)

### Phase 5 — Tab UI in nize-desktop Frontend

Convert the main view to a tabbed layout.

- [ ] **5.1** Create `packages/nize-desktop/src/TabView.tsx`:
  - Generic n-tab component: accepts `tabs: Array<{ id: string; label: string; content: ReactNode; disabled?: boolean }>`
  - Renders tab bar from the array — supports any number of tabs
  - Active tab controlled by React state; inactive panels hidden (not unmounted, to preserve iframe)
  - Caller assembles tabs array: `[{ id: "desktop", label: "Desktop", content: <MainAppContent /> }, { id: "web", label: "Web", content: <iframe … />, disabled: !port }]`
- [ ] **5.2** Update `AuthGate.tsx` or `MainApp.tsx`:
  - Fetch `nize_web_port` via `invoke<number>("get_nize_web_port")`
  - Wrap existing content in `<TabView nizeWebPort={port}>`
- [ ] **5.3** Style the tabs:
  - Tab bar: horizontal row at the top, below the header
  - Active tab: visually distinct (bold / underline / background)
  - Tab content: fills remaining viewport height
  - iframe: `width: 100%; height: 100%; border: none;` inside a scrollable container
- [ ] **5.4** Handle states:
  - If nize-web port is null (sidecar not started), disable the "Web" tab or show a message
  - Loading state: show spinner while iframe loads

### Phase 6 — Build Integration

Wire everything together for `cargo tauri dev` and `cargo tauri build`.

- [ ] **6.1** Update `scripts/setup-sidecar-binaries.sh` if needed (no change expected — nize-web uses node, not a new binary)
- [ ] **6.2** Add `build:nize-web` script to `packages/nize-desktop/package.json`
- [ ] **6.3** Verify `cargo tauri dev` launches all sidecars and the tab UI works
- [ ] **6.4** Verify `cargo tauri build` bundles nize-web resources correctly

## Risks

| Risk | Mitigation |
|------|------------|
| Next.js standalone output size (~15–30 MB) increases bundle | Monitor size; acceptable tradeoff for full React SSR |
| Port conflicts between sidecars | All use ephemeral port 0; no conflict possible |
| iframe CSP/CORS issues | CSP is already null; Next.js serves from localhost so no CORS |
| Dev mode: nize-web not built before desktop starts | beforeDevCommand ensures build order |
| Next.js cold-start time adds to app launch | Start nize-web in parallel with other sidecars; iframe shows loading state |

## Open Questions

1. ~~Should the nize-web sidecar share the API port/auth with nize_desktop_server?~~ — **Resolved**: nize-web will communicate with nize_desktop_server later (not yet in scope). Ephemeral port confirmed.
2. **Should the tab state persist across app restarts?** — Not for v1. Simple React state is sufficient.
3. ~~Any preferred Next.js version?~~ — **Resolved**: Latest stable — Next.js 16.1.x.

## Completion Criteria

- [ ] `packages/nize-web` exists with a working Next.js hello world app
- [ ] nize-web runs as a sidecar via the bundled node binary
- [ ] nize-desktop shows a tab bar with "Desktop" and "Web" tabs
- [ ] "Desktop" tab shows existing content unchanged
- [ ] "Web" tab loads nize-web in a scrollable iframe
- [ ] `cargo tauri dev` and `cargo tauri build` both work end-to-end
