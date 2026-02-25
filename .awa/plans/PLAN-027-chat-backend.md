# PLAN-027: Chat Backend — nize-chat Package + Hono-in-Next.js

**Status:** in-progress
**Workflow direction:** lateral
**Traceability:** ARCHITECTURE.md → nize-web, nize_api components; PLAN-026 (frontend port)

## Goal

Implement a chat backend as a new `packages/nize-chat` workspace package with Hono, mounted inside nize-web via `@hono/nextjs`. Simplified from the ref project — no RAG, no agent orchestrator, no instruction sets — but retaining compaction (including tool output summarization for MCP tool responses) and model-agnostic streaming via the AI SDK.

**In scope:** Chat service, compaction, config fetching, Hono app, Next.js adapter, new Rust API endpoints for message persistence.
**Out of scope:** RAG/search, agent orchestration, instruction sets, tool policy, trace emission, audit logging. Tool *calling* is deferred but compaction must handle tool output parts when they arrive.

## Architecture Decision

| Considered | Decision | Rationale |
|---|---|---|
| (A) Rust in nize_api | Rejected | No Rust AI SDK; manual streaming protocol reimplementation |
| (B) Next.js Route Handler only | Rejected | Chat logic buried in Next.js App Router; less portable |
| (B') Separate package + Hono-in-Next.js | **Selected** | Portable Hono app; AI SDK native; no new processes; ref-compatible patterns |
| (C) Standalone TS on Bun | Rejected | Extra sidecar; overkill for one endpoint |

## Component Layout

```
packages/nize-chat/                    ← NEW workspace package
  package.json                         { "name": "@six5536/nize-chat" }
  tsconfig.json
  vitest.config.ts
  src/
    index.ts                           # barrel exports
    app.ts                             # Hono app definition (chat routes)
    chat-service.ts                    # processChat() → streamText → stream response
    chat-config.ts                     # fetch agent.* config from Rust API
    compaction.ts                      # maybeCompact + ToolOutputSummarizer
    types.ts                           # ChatRequest, ChatConfig, CompactState, etc.
  tests/
    compaction.test.ts                 # ported from ref @nize/agent tests
    chat-service.test.ts

packages/nize-web/                     ← EXISTING (adapter wiring only)
  app/api/chat/route.ts                # @hono/nextjs handle() → nize-chat app
```

## Data Flow

```
Frontend (useChat + DefaultChatTransport → /api/chat)
  └─ Next.js Route Handler (app/api/chat/route.ts) → @hono/nextjs → nize-chat Hono app
       ├─ Auth: forward cookie to Rust API, validate
       ├─ Config: GET /config → agent.model.name, agent.compaction.maxMessages
       ├─ Conversation: GET/POST /conversations → validate/create
       ├─ Compaction: maybeCompact(messages, maxMessages)
       │    └─ ToolOutputSummarizer: truncate large MCP tool response parts
       ├─ LLM call: streamText({ model, messages }) → AI SDK
       ├─ Stream: toUIMessageStreamResponse() → SSE back to frontend
       └─ onFinish:
            ├─ PUT /conversations/{id}/messages → Rust API (persist)
            └─ PATCH /conversations/{id} → Rust API (auto-title)
```

## Key Design Points

### 1. Route Mount: `/api/chat` (Next.js Route Handler precedence)

nize-web's `next.config.ts` rewrites `/api/:path*` → Rust API. However, Next.js Route Handlers take precedence over rewrites — if `app/api/chat/route.ts` exists, Next.js serves it directly without consulting the rewrite rules. This means no `next.config.ts` changes are needed.

The Hono app uses `basePath("/api")` and defines `POST /chat`. The frontend uses `apiUrl("/chat")` → `/api/chat`, which is identical to the ref project's URL pattern.

The Rust API's demo stub at `POST /api/chat` becomes unreachable from nize-web (the Route Handler intercepts first), but remains available for direct API calls.

### 2. Auth Forwarding

The Hono handler receives the auth cookie (same-origin via Next.js). It forwards it to the Rust API when calling config/conversation endpoints. The nize-chat package does NOT hold the JWT secret — auth validation is delegated to the Rust API.

### 3. Compaction with Tool Output Summarization

Ported from `@nize/agent` `compaction.ts`. Includes `ToolOutputSummarizer` because MCP tool responses can be large (JSON payloads). The compactor:

1. Summarizes large tool output parts (>100KB) in assistant messages — truncates JSON arrays/objects intelligently
2. When message count exceeds `compactionMaxMessages`, takes last N messages + generates a text summary of older ones
3. Summary is prepended as a system message for context continuity

The `Message` type for compaction is simplified: `{ role, content }` text-only — `UIMessage` parts are flattened to text for compaction purposes, preserving original `UIMessage[]` when compaction hasn't triggered.

### 4. Model Selection

Config key `agent.model.name` (format: `provider:model`, e.g. `anthropic:claude-haiku-4-5-20251001`).

Supported providers: `anthropic` (`@ai-sdk/anthropic`), `openai` (`@ai-sdk/openai`), `google` (`@ai-sdk/google`).

Falls back to env var `CHAT_MODEL`, then config default.

### 5. Message Persistence

**New Rust API endpoint needed:** `PUT /conversations/{id}/messages`

Accepts `UIMessage[]`, bulk-inserts into `messages` table. Called from `onFinish` callback after stream completes. This keeps persistence in the Rust API (single source of truth for DB writes).

Current conversation endpoints are demo stubs — they need real implementations before chat works end-to-end. This plan covers only the nize-chat package and its adapter; conversation CRUD implementation is a separate task.

### 6. Title Generation

Done inside `processChat()` — a separate `streamText` call with a short title-generation prompt. Updates title via `PATCH /conversations/{id}`. Only triggers on the first user message when title is "New Chat".

## Steps

### Phase 1: Package Scaffold

- [x] 1.1 — Create `packages/nize-chat/package.json` with dependencies:
  - `hono`, `ai`, `@ai-sdk/anthropic`, `@ai-sdk/openai`, `@ai-sdk/google`
  - devDeps: `vitest`, `@types/node`, `typescript`
- [x] 1.2 — Create `tsconfig.json` (ESM, strict, paths)
- [x] 1.3 — Create `vitest.config.ts`
- [x] 1.4 — Create `src/index.ts` barrel

### Phase 2: Types & Config

- [x] 2.1 — Create `src/types.ts`: `ChatRequest`, `ChatConfig`, `CompactMessage`, `CompactState`, `ContextSummary`
- [x] 2.2 — Create `src/chat-config.ts`: `fetchChatConfig(apiBaseUrl, cookie)` → fetches `agent.model.name`, `agent.model.temperature`, `agent.compaction.maxMessages` from Rust API

### Phase 3: Compaction

- [x] 3.1 — Create `src/compaction.ts`: port `ToolOutputSummarizer`, `Compactor`, `maybeCompact` from ref `@nize/agent`. Simplify types (no `MetaInstruction`/`InstructionRefs`/`ContextPack` — just `CompactState` with messages + optional summary)
- [x] 3.2 — Create `tests/compaction.test.ts`: port relevant tests from ref

### Phase 4: Chat Service

- [x] 4.1 — Create `src/chat-service.ts`: `processChat(request, config, apiBaseUrl, cookie)`:
  1. Get/create conversation via Rust API
  2. Flatten UIMessages to CompactMessages
  3. maybeCompact()
  4. Build model messages (compacted text or `convertToModelMessages`)
  5. `streamText()` with configured model
  6. Return `{ toUIMessageStreamResponse(), conversationId, isNew }`
  7. `onFinish` → persist messages + generate title via Rust API
- [x] 4.2 — Create `src/model-registry.ts`: `getChatModel(spec)` — returns AI SDK model instance from `provider:model` string

### Phase 5: Hono App

- [x] 5.1 — Create `src/app.ts`: Hono app with:
  - `POST /chat` handler: extract auth cookie, fetch config, call `processChat()`, return stream response
  - Error handling (catch block in handler)

### Phase 6: Next.js Adapter

- [x] 6.1 — Add `@six5536/nize-chat` to `packages/nize-web/package.json` (no `@hono/nextjs` needed — Hono uses Web Standard Request/Response directly)
- [x] 6.2 — Create `packages/nize-web/app/api/chat/route.ts`:
  ```ts
  import { chatApp } from "@six5536/nize-chat";
  export const POST = (request: Request) => chatApp.fetch(request);
  ```
  Next.js Route Handlers take precedence over `next.config.ts` rewrites, so `/api/chat` is served locally while all other `/api/*` paths continue proxying to the Rust API. No rewrite changes needed.
- [x] 6.3 — Frontend `DefaultChatTransport` API URL: `apiUrl("/chat")` → `/api/chat`. Already wired in existing frontend.

### Phase 7: Rust API — Message Persistence Endpoint

- [x] 7.1 — Add `PUT /conversations/{id}/messages` to TypeSpec contract (`API-NIZE-conversations.tsp`)
- [x] 7.2 — Regenerate API code (`bun run generate:api`)
- [x] 7.3 — Implement handler in `nize_api` (even as a stub initially — real impl requires conversation CRUD to be real first)

### Phase 8: Validation

- [x] 8.1 — Run `bun install` from workspace root
- [x] 8.2 — Run nize-chat tests: `cd packages/nize-chat && bun run test` (12/12 pass)
- [x] 8.3 — Run nize-web build: `cd packages/nize-web && bun run build`
- [ ] 8.4 — Manual smoke test: start `cargo tauri dev`, open chat, verify stream renders (will fail until conversation CRUD is real — expected)

## Dependencies

| Dependency | Status | Blocking? |
|---|---|---|
| PLAN-026 (chat frontend port) | in-progress | Yes — frontend must exist to test |
| Conversation CRUD (real impl) | not started | Partially — chat will fail at persistence but can stream |
| Config system (agent.* keys seeded) | done | No — migration 0003 already seeds these |

## Risks

| Risk | Mitigation |
|---|---|
| `@hono/nextjs` compatibility with Next.js 16 standalone output | Hono's Next.js adapter is well-maintained; test early |
| AI SDK streaming protocol changes | Pin `ai` version; streaming wire format is stable |
| Large MCP tool responses exceed compaction limits | ToolOutputSummarizer truncates to 100KB per output; configurable |
| Route Handler precedence assumption wrong | Documented Next.js behavior; verified in Next.js 16 docs. Add integration test to confirm. |
| Conversation CRUD stubs prevent end-to-end testing | Accept partial testing; streaming + compaction can be unit-tested independently |

## Completion Criteria

- `packages/nize-chat` exists as workspace package with Hono app exporting `chatApp`
- `processChat()` streams AI SDK responses with model from config
- Compaction triggers at `compactionMaxMessages` threshold with tool output summarization
- nize-web Route Handler at `app/api/chat/route.ts` intercepts `/api/chat` (precedence over rewrite)
- Frontend `DefaultChatTransport` points to `/api/chat` (same as ref project)
- Compaction tests pass
- `bun run build` succeeds in nize-web
