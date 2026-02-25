# PLAN-023: Admin Embeddings Settings & Search UI

**Status:** done
**Workflow direction:** bottom-up
**Traceability:** PLAN-022 (text embedding generation infrastructure); PLAN-020 (MCP service registration — admin tools UI pattern); PLAN-021 (Next.js as Tauri frontend — settings layout)

## Goal

Add two admin pages under Settings for embedding management:
1. **Embeddings Config** (`/settings/admin/embeddings`) — configure provider/model, view registered models, re-index all tools
2. **Embeddings Search** (`/settings/admin/embeddings/search`) — test semantic search with raw similarity scores (0–1)

## Context

PLAN-022 delivered the embedding infrastructure in `nize_core::embedding`:
- `EmbeddingConfig` resolves from admin config system (`embedding.provider`, `embedding.activeModel`, `embedding.ollamaBaseUrl`, `embedding.openaiApiKey`)
- `embedding_models` DB table lists registered models per provider
- `tool_embeddings_*` tables store per-tool vectors for semantic search
- `indexer::embed_server_tools()` generates and stores embeddings on tool registration
- Providers: OpenAI, Ollama, local (deterministic FNV)

Existing config admin API:
- `GET /api/admin/config?category=embedding` — returns embedding config definitions with current values
- `PATCH /api/admin/config/{scope}/{key}` — updates a config value

No API endpoint exists yet for:
- Listing registered embedding models from `embedding_models` table
- Performing a similarity search against tool embeddings
- Re-indexing all server tools

The UI follows the pattern established in PLAN-020 (`/settings/admin/tools`) and PLAN-021 (`/settings/layout.tsx` with sidebar nav).

## Scope

### In-scope

1. **Backend**: Three new admin API endpoints in `nize_api`:
   - `GET /admin/embeddings/models` — list registered embedding models
   - `POST /admin/embeddings/search` — embed a query and return ranked tool matches (raw 0–1 scores)
   - `POST /admin/embeddings/reindex` — re-index all tools across all servers
2. **Frontend**: Two pages:
   - `/settings/admin/embeddings` — config panel + models table + re-index button
   - `/settings/admin/embeddings/search` — query input + ranked results with raw similarity scores
3. **Navigation**: Add "Embeddings" and "Embedding Search" links to admin nav in settings layout

### Out-of-scope

- Embedding model CRUD (models are seeded in migration)
- Chunk embeddings / document search
- TypeSpec contract updates (endpoints added directly as non-generated routes)

## Backend Design

### Handler: `crates/lib/nize_api/src/handlers/embeddings.rs`

```rust
/// GET /admin/embeddings/models — list registered embedding models
pub async fn list_models_handler(State(state): State<AppState>) -> AppResult<Json<Value>>

/// POST /admin/embeddings/search — test embedding similarity search
pub async fn search_handler(State(state): State<AppState>, Json(body): Json<SearchRequest>) -> AppResult<Json<Value>>

/// POST /admin/embeddings/reindex — re-index all server tools
pub async fn reindex_handler(State(state): State<AppState>) -> AppResult<Json<Value>>
```

#### Search request/response

```rust
struct SearchRequest {
    query: String,         // text to embed and search with
    limit: Option<i32>,    // max results (default 10)
}

// Response: { results: [{ toolName, serverName, domain, similarity }] }
// similarity is raw 0–1 cosine score
```

#### Search implementation

1. Resolve `EmbeddingConfig` from admin config system
2. Get active model config (`models::get_active_model`)
3. Embed the query text via `provider::embed_with_model`
4. Run cosine similarity query against the active model's tool embedding table:
   ```sql
   SELECT t.name, s.name AS server_name, te.domain,
          1 - (te.embedding <=> $1::vector) AS similarity
   FROM "{tool_table_name}" te
   JOIN mcp_server_tools t ON t.id = te.tool_id
   JOIN mcp_servers s ON s.id = te.server_id
   ORDER BY te.embedding <=> $1::vector
   LIMIT $2
   ```
5. Return ranked results with raw similarity scores

#### Reindex implementation

1. Resolve `EmbeddingConfig`
2. Iterate all MCP servers via `mcp::queries::list_all_servers`
3. Call `indexer::embed_server_tools()` for each server
4. Return `{ indexed: N, errors: [...] }`

### Route registration

Add to admin router in `nize_api::router()` using hardcoded paths:

```rust
.route("/admin/embeddings/models", get(embeddings::list_models_handler))
.route("/admin/embeddings/search", post(embeddings::search_handler))
.route("/admin/embeddings/reindex", post(embeddings::reindex_handler))
```

## Frontend Design

### Page 1: `/settings/admin/embeddings/page.tsx` — Configuration

Two sections:

#### 1. Embedding Configuration

- Fetches `GET /api/admin/config?category=embedding`
- Renders each config item with appropriate input (selector for provider, text for URLs/keys)
- Updates via `PATCH /api/admin/config/system/{key}`
- Follows inline style pattern from `/settings/page.tsx`

#### 2. Registered Models + Re-index

- Fetches `GET /api/admin/embeddings/models`
- Table with columns: Provider, Model Name, Dimensions, Tool Table
- Shows which model is currently active (matches `embedding.activeModel` config)
- "Re-index All Tools" button → `POST /api/admin/embeddings/reindex`
- Shows indexing progress/result (count indexed, any errors)

### Page 2: `/settings/admin/embeddings/search/page.tsx` — Search Tester

- Text input for query
- "Search" button → `POST /api/admin/embeddings/search`
- Results table: Tool Name, Server Name, Domain, Similarity (raw 0–1)
- Loading/error states
- Empty state when no embeddings exist yet

### Navigation update: `packages/nize-web/app/settings/layout.tsx`

Add to `adminNavItems`:
```tsx
{ href: "/settings/admin/embeddings", label: "Embeddings" },
{ href: "/settings/admin/embeddings/search", label: "Embedding Search" },
```

## Implementation Steps

### Step 1: Backend — Embeddings handler module

Create `crates/lib/nize_api/src/handlers/embeddings.rs`:
- `list_models_handler` — queries `embedding_models` table
- `search_handler` — embeds query, runs similarity search, returns raw scores
- `reindex_handler` — iterates servers, calls embed_server_tools for each

### Step 2: Backend — Register handler and routes

- Add `pub mod embeddings;` to `handlers/mod.rs`
- Add routes to admin router in `lib.rs`

### Step 3: Backend — Build verification

- `cargo build -p nize_api` compiles
- `cargo clippy -p nize_api` no warnings

### Step 4: Frontend — Create config page

Create `packages/nize-web/app/settings/admin/embeddings/page.tsx`:
- Config section using admin config API
- Models table + re-index button using new endpoints

### Step 5: Frontend — Create search page

Create `packages/nize-web/app/settings/admin/embeddings/search/page.tsx`:
- Query input + search button
- Results table with raw similarity scores

### Step 6: Frontend — Wire navigation

Update `adminNavItems` in `packages/nize-web/app/settings/layout.tsx`.

### Step 7: Verification

- `cargo build` succeeds
- Config page renders at `/settings/admin/embeddings`
- Search page renders at `/settings/admin/embeddings/search`
- Config section loads embedding settings
- Models table displays registered models
- Re-index button triggers indexing
- Search returns results with raw similarity scores

## Dependencies

- PLAN-022 infrastructure (embedding module, migration, config definitions) — **completed**
- Existing admin config API (`GET /admin/config`, `PATCH /admin/config/{scope}/{key}`) — **available**
- `nize_core::embedding` module (config, models, provider, indexer) — **available**

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| No tool embeddings indexed yet | Search page shows empty state; re-index button available on config page |
| Embedding provider not configured / unavailable | Show provider status; search/reindex endpoints return clear errors |
| Dynamic SQL table name in search query | Validate table name from `embedding_models` row (trusted DB data) |
| Large embedding vectors in HTTP response | Only return similarity scores, not raw vectors |
| Re-index takes long for many servers | Return summary after completion; future: async with progress |

## Completion Criteria

- [x] `GET /admin/embeddings/models` returns registered models
- [x] `POST /admin/embeddings/search` returns ranked tool matches with raw 0–1 similarity
- [x] `POST /admin/embeddings/reindex` re-indexes all server tools
- [x] Admin nav shows "Embeddings" and "Embedding Search" links
- [x] Config page displays and edits `embedding.*` settings
- [x] Config page shows models table and re-index button
- [x] Search page accepts query and displays ranked results with raw scores
- [x] `cargo build` and `cargo clippy` pass
- [x] Pages accessible only to admin users (guarded by existing admin layout)
