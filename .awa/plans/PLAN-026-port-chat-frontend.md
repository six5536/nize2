# PLAN-026: Port Chat Frontend from Reference Project

**Status:** in-progress
**Workflow direction:** lateral
**Traceability:** ARCHITECTURE.md → nize-web component

## Goal

Port the complete chat frontend from the reference project (`submodules/nize/apps/web`) into `packages/nize-web`, copying code exactly as-is to preserve UI fixes.

**In scope:** Front-end chat UI only.
**Out of scope:** Backend chat handler implementation (currently a demo stub in `nize_api`).

## Current State Analysis

### Reference project (`submodules/nize/apps/web`)

Full chat UI with:
- Chat components: `chat-header`, `chat-input`, `chat-layout`, `chat-upload`, `code-block`, `message-actions`, `message-bubble`, `tool-renderer`, barrel `index.ts`
- Chat hooks: `use-chat-handlers`, `use-chat-scroll`
- Dev panel: `dev-panel-provider`, `dev-panel-toggle`, `dev-panel`, `resize-handle`, `tab-bar`, tabs (`chat-trace-tab`, `raw-chat-tab`), barrel `index.ts`
- UI components: `button`, `collapsible`, `tooltip`
- Sidebar: `conversation-item`, `conversation-list`, `delete-confirm-dialog`, `new-chat-button`, `sidebar-toggle`, `sidebar` (all present in nize-web already)
- Lib: `api`, `auth-context`, `dev-panel-context`, `errors`, `message-parts`, `streamdown-config`, `types`, `utils`
- App pages: `chat/page.tsx`, `chat/[conversationId]/page.tsx`, `layout.tsx` (with DevPanel), `middleware.ts`
- Tests: `code-block.test.tsx`, `message-actions.test.tsx`, `message-bubble.test.tsx`, `tool-renderer.test.tsx`, `setup.ts`, `vitest.config.ts`

Key dependencies: `@ai-sdk/react`, `ai` (AI SDK v6), `streamdown`, `@streamdown/code`, `sonner`, `lucide-react`, `clsx`, `tailwind-merge`, `class-variance-authority`, `usehooks-ts`, `throttleit`, `@radix-ui/react-collapsible`, `@radix-ui/react-tooltip`, `swr`, `nanoid`

### Current nize-web (`packages/nize-web`)

- Chat: Only `chat-layout.tsx` (identical to ref). Conversation page is a "coming soon" placeholder.
- No chat components, hooks, dev panel, UI components, or related libs (`message-parts`, `streamdown-config`, `errors`, `utils`, `dev-panel-context`)
- No `middleware.ts`
- Missing npm dependencies for chat functionality
- Has Tauri-specific files not in ref: `WebviewBridgeLoader.tsx`, `lib/tauri.ts`, `lib/webview-bridge.ts`, `lib/auth-gate.ts`, `proxy.ts`, desktop components
- `lib/api.ts` handles Tauri IPC port discovery (more complex than ref — keep nize-mcp version)
- `lib/auth-context.tsx` already adapted for nize-mcp (awa markers, slight differences — keep nize-mcp version)
- `lib/types.ts` has subset of ref types (missing dev panel types)
- No tests or vitest config

## Key Adaptation Considerations

1. **`lib/api.ts` — KEEP nize-mcp version.** Ref uses simple `process.env.NEXT_PUBLIC_API_URL` fallback. nize-mcp has Tauri IPC discovery, `window.__NIZE_ENV__`, and Next.js rewrite proxying. The ref version is incompatible.

2. **`lib/auth-context.tsx` — KEEP nize-mcp version.** Already adapted; functionally equivalent to ref but with nize-mcp awa markers and minor wording tweaks.

3. **`lib/types.ts` — MERGE.** Add dev panel types from ref into existing nize-mcp file.

4. **`app/layout.tsx` — MERGE.** Add `DevPanelProvider`, `DevPanel`, `Toaster` wrapping from ref layout into existing nize-mcp layout (which has `WebviewBridgeLoader` and `__nize-env.js` script).

5. **`app/chat/[conversationId]/page.tsx` — COPY from ref.** Replace placeholder. The ref version uses `@ai-sdk/react`, `useChat`, `DefaultChatTransport`, `useDevPanel` — all matching the ref backend's AI SDK streaming response.

6. **`middleware.ts` — COPY from ref.** nize-mcp has no middleware yet.

7. **Sidebar files — DIFF and port changes.** Both projects have the same sidebar files. Need to check for UI fixes in the ref versions and copy if different.

8. **Chat endpoint compatibility.** The ref's conversation page calls `/chat` with AI SDK's `DefaultChatTransport` which uses AI SDK's streaming protocol. The nize-mcp backend's `POST /chat` is currently a demo stub returning simple JSON. The ported frontend will render but the chat submit will fail until the backend is implemented. This is acceptable (out of scope).

## Steps

### Phase 1: Dependencies

- [ ] 1.1 — Add missing npm dependencies to `packages/nize-web/package.json`:
  - `@ai-sdk/react`, `ai` (AI SDK)
  - `streamdown`, `@streamdown/code` (markdown rendering)
  - `sonner` (toast notifications)
  - `lucide-react` (icons)
  - `clsx`, `tailwind-merge`, `class-variance-authority` (styling utils)
  - `usehooks-ts`, `throttleit` (hooks/utils)
  - `@radix-ui/react-collapsible`, `@radix-ui/react-tooltip` (UI primitives)
  - `swr` (data fetching for sidebar)
  - `nanoid` (ID generation)
- [ ] 1.2 — Add test devDependencies: `vitest`, `@vitejs/plugin-react`, `@testing-library/react`, `@testing-library/dom`, `@testing-library/jest-dom`, `jsdom`, `fast-check`
- [ ] 1.3 — Run `bun install` from workspace root

### Phase 2: Lib Files

- [ ] 2.1 — Copy new lib files from ref (exact copy):
  - `lib/errors.ts`
  - `lib/message-parts.ts`
  - `lib/streamdown-config.tsx`
  - `lib/utils.ts`
  - `lib/dev-panel-context.ts`
- [ ] 2.2 — Merge `lib/types.ts`: add dev panel types (`DevPanelState`, `ChatStateData`, `TabDefinition`, `DEV_PANEL_STORAGE_KEY`, panel width/height constants) from ref

### Phase 3: UI Components

- [ ] 3.1 — Copy `components/ui/` from ref (exact):
  - `button.tsx`
  - `collapsible.tsx`
  - `tooltip.tsx`

### Phase 4: Chat Components

- [ ] 4.1 — Replace `components/chat/chat-layout.tsx` with ref version (identical, no change expected)
- [ ] 4.2 — Copy new chat components from ref (exact):
  - `chat-header.tsx`
  - `chat-input.tsx`
  - `chat-upload.tsx`
  - `code-block.tsx`
  - `message-actions.tsx`
  - `message-bubble.tsx`
  - `tool-renderer.tsx`
  - `index.ts` (barrel export)
- [ ] 4.3 — Copy chat hooks from ref (exact):
  - `hooks/use-chat-handlers.ts`
  - `hooks/use-chat-scroll.ts`

### Phase 5: Dev Panel Components

- [ ] 5.1 — Copy `components/dev-panel/` from ref (exact):
  - `dev-panel-provider.tsx`
  - `dev-panel-toggle.tsx`
  - `dev-panel.tsx`
  - `resize-handle.tsx`
  - `tab-bar.tsx`
  - `index.ts`
  - `tabs/chat-trace-tab.tsx`
  - `tabs/raw-chat-tab.tsx`

### Phase 6: Sidebar Updates

- [ ] 6.1 — Diff each sidebar file between ref and nize-web; copy ref versions for any files with UI fixes

### Phase 7: App Pages & Layout

- [ ] 7.1 — Copy `middleware.ts` from ref
- [ ] 7.2 — Merge `app/layout.tsx`: add `DevPanelProvider`, `DevPanel`, `Toaster` from ref while preserving nize-mcp's `WebviewBridgeLoader` and `__nize-env.js`
- [ ] 7.3 — Copy `app/chat/page.tsx` from ref (compare first; may have fixes)
- [ ] 7.4 — Copy `app/chat/[conversationId]/page.tsx` from ref (replaces placeholder)

### Phase 8: Tests

- [ ] 8.1 — Copy test files from ref (exact):
  - `tests/setup.ts`
  - `tests/code-block.test.tsx`
  - `tests/message-actions.test.tsx`
  - `tests/message-bubble.test.tsx`
  - `tests/tool-renderer.test.tsx`
- [ ] 8.2 — Copy `vitest.config.ts` from ref
- [ ] 8.3 — Add `"test": "vitest run"` script to `package.json`

### Phase 9: Validation

- [ ] 9.1 — Run `bun install` and verify no resolution errors
- [ ] 9.2 — Run `bun run build` in `packages/nize-web` to verify compilation
- [ ] 9.3 — Run tests: `bun run test` in `packages/nize-web`

## Risks

| Risk | Mitigation |
|------|-----------|
| AI SDK streaming protocol mismatch with demo backend stub | Expected — backend is out of scope. Chat submit will fail gracefully until backend serves AI SDK streaming responses. |
| Ref's `auth-context` uses `nize_user` localStorage key; nize-mcp uses `nize_user` too | Already aligned — no issue. |
| Dev panel trace tab calls `/dev/chat_trace` which is a demo stub | Acceptable — trace tab will show empty/error state until backend is implemented. |
| Package version mismatches between ref and nize-mcp workspace | Pin to same versions as ref `package.json`. |
| Ref uses `@nize/agent`, `@nize/db`, etc. workspace packages | These are backend-only deps not used by front-end chat code. Not needed. |

## Completion Criteria

- All chat components, hooks, dev panel, UI components, and lib files from ref are present in `packages/nize-web`
- `app/layout.tsx` includes DevPanel wrapper and Toaster
- `app/chat/[conversationId]/page.tsx` renders full chat UI (not placeholder)
- `middleware.ts` handles route protection
- `bun run build` succeeds in `packages/nize-web`
- Tests pass (or are skipped with documented reason if backend mocking is needed)
