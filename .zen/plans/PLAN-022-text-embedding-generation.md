# PLAN-022: Text Embedding Generation for MCP Servers

**Status:** in-progress
**Workflow direction:** bottom-up
**Traceability:** Reference project `submodules/nize/` — `packages/ingestion/src/embedder.ts`, `packages/agent/src/mcp/tool-index.ts`, `packages/db/src/schema/embeddings.ts`, `packages/db/src/schema/documents.ts`; PLAN-019 (MCP semantic tool discovery stubs); PLAN-020 (MCP service registration)

## Goal

Implement a complete text embedding generation layer in Rust (`nize_core`) supporting multiple providers (OpenAI, Ollama, local/deterministic) — matching the reference project's `@nize/ingestion` embedder. This layer will be used by the MCP tool discovery system (PLAN-019) to embed tool descriptions into pgvector for semantic search. Hooking embeddings into the MCP tool discovery interface is **out of scope**; only the embedding infrastructure itself is delivered.

## Context

The reference project implements embeddings in TypeScript (`packages/ingestion/src/embedder.ts`) with:
- An `embedding_models` DB registry mapping provider+model → dimensions + table name
- Per-model embedding tables using pgvector `VECTOR(N)` columns (chunk embeddings + tool embeddings)
- Three providers: `openai` (via OpenAI API), `ollama` (via local Ollama HTTP API), `local` (deterministic FNV hash)
- Config via env vars: `EMBEDDING_PROVIDER`, `EMBEDDING_ACTIVE_MODEL`, `OLLAMA_BASE_URL`, `OPENAI_API_KEY`
- Functions: `embed(texts)`, `embedSingle(text)`, `getEmbeddingModelConfigs()`, `getActiveEmbeddingModel()`

In nize-mcp:
- pgvector extension is already enabled in `nize_core::db` (migration + `CREATE EXTENSION IF NOT EXISTS vector`)
- `mcp_servers` and `mcp_server_tools` tables exist (migration 0004)
- The `nize_mcp` crate has stub meta-tools from PLAN-019 that will eventually call this embedding layer
- SQLx with compile-time query checking is the DB access pattern
- `reqwest` is already a workspace dependency (for Ollama/OpenAI HTTP calls)

## Scope

### In-scope

1. **Database migration** (`0005_embedding_models.sql`) — `embedding_models` registry table + per-model tool embedding tables (pgvector)
2. **Rust embedding module** (`nize_core::embedding`) — provider abstraction, embed/embedSingle, model config queries
3. **Three providers**: OpenAI (`text-embedding-3-small`), Ollama (`nomic-embed-text`), local (deterministic)
4. **Config integration** — embedding provider/model selection via nize config system or env vars
5. **Unit tests** — local provider, config resolution, embedding dimensions validation

### Out-of-scope

- Wiring embeddings into MCP tool discovery (PLAN-019 stubs → real search)
- Chunk embeddings for document ingestion (future)
- REST API endpoints for embedding management
- Embedding model CRUD (models are seeded in migration)
- Search/similarity queries (will be added when wiring to discovery)

## Database Schema

### Migration: `0005_embedding_models.sql`

```sql
-- Embedding models registry (matches ref: packages/db/src/schema/documents.ts)
CREATE TABLE IF NOT EXISTS embedding_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider VARCHAR(50) NOT NULL,
    name VARCHAR(100) NOT NULL,
    table_name VARCHAR(120) NOT NULL,
    tool_table_name VARCHAR(120) NOT NULL,
    dimensions INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS embedding_models_provider_name_idx
    ON embedding_models(provider, name);
CREATE UNIQUE INDEX IF NOT EXISTS embedding_models_table_name_idx
    ON embedding_models(table_name);
CREATE UNIQUE INDEX IF NOT EXISTS embedding_models_tool_table_name_idx
    ON embedding_models(tool_table_name);

-- Seed models (matches ref migration 0001_initial.sql)
INSERT INTO embedding_models (provider, name, table_name, tool_table_name, dimensions)
VALUES
    ('openai', 'text-embedding-3-small',
     'chunk_embeddings_openai_text_embedding_3_small',
     'tool_embeddings_openai_text_embedding_3_small',
     1536),
    ('ollama', 'nomic-embed-text',
     'chunk_embeddings_ollama_nomic_embed_text',
     'tool_embeddings_ollama_nomic_embed_text',
     768)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Seed embedding config definitions (admin-configurable)
-- ---------------------------------------------------------------------------

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description, possible_values, validators)
VALUES (
    'embedding.provider',
    'embedding',
    'string',
    'selector',
    'ollama',
    'Embedding Provider',
    'The embedding provider to use (openai requires API key, ollama requires local server, local is deterministic/offline)',
    '["openai","ollama","local"]'::jsonb,
    '[{"type":"required","message":"Embedding provider is required"}]'::jsonb
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    possible_values = EXCLUDED.possible_values,
    validators = EXCLUDED.validators;

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description, validators)
VALUES (
    'embedding.activeModel',
    'embedding',
    'string',
    'text',
    'nomic-embed-text',
    'Active Embedding Model',
    'The embedding model to use (must match a registered model in embedding_models table)',
    '[{"type":"required","message":"Active model is required"}]'::jsonb
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    validators = EXCLUDED.validators;

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description, validators)
VALUES (
    'embedding.ollamaBaseUrl',
    'embedding',
    'string',
    'text',
    'http://localhost:11434',
    'Ollama Base URL',
    'Base URL for the Ollama API server',
    '[{"type":"required","message":"Ollama base URL is required"}]'::jsonb
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    validators = EXCLUDED.validators;

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'embedding.openaiApiKey',
    'embedding',
    'string',
    'text',
    '',
    'OpenAI API Key',
    'API key for OpenAI embeddings (required when provider is openai)'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;

-- ---------------------------------------------------------------------------
-- Tool embedding tables (one per model, for MCP tool semantic discovery)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS tool_embeddings_openai_text_embedding_3_small (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tool_id UUID NOT NULL REFERENCES mcp_server_tools(id) ON DELETE CASCADE,
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    domain TEXT NOT NULL,
    embedding VECTOR(1536) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS tool_embeddings_openai_te3s_tool_idx
    ON tool_embeddings_openai_text_embedding_3_small(tool_id);
CREATE INDEX IF NOT EXISTS tool_embeddings_openai_te3s_server_idx
    ON tool_embeddings_openai_text_embedding_3_small(server_id);
CREATE INDEX IF NOT EXISTS tool_embeddings_openai_te3s_domain_idx
    ON tool_embeddings_openai_text_embedding_3_small(domain);
CREATE INDEX IF NOT EXISTS tool_embeddings_openai_te3s_embedding_idx
    ON tool_embeddings_openai_text_embedding_3_small
    USING hnsw (embedding vector_cosine_ops);

CREATE TABLE IF NOT EXISTS tool_embeddings_ollama_nomic_embed_text (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tool_id UUID NOT NULL REFERENCES mcp_server_tools(id) ON DELETE CASCADE,
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    domain TEXT NOT NULL,
    embedding VECTOR(768) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS tool_embeddings_ollama_net_tool_idx
    ON tool_embeddings_ollama_nomic_embed_text(tool_id);
CREATE INDEX IF NOT EXISTS tool_embeddings_ollama_net_server_idx
    ON tool_embeddings_ollama_nomic_embed_text(server_id);
CREATE INDEX IF NOT EXISTS tool_embeddings_ollama_net_domain_idx
    ON tool_embeddings_ollama_nomic_embed_text(domain);
CREATE INDEX IF NOT EXISTS tool_embeddings_ollama_net_embedding_idx
    ON tool_embeddings_ollama_nomic_embed_text
    USING hnsw (embedding vector_cosine_ops);
```

Note: Chunk embedding tables are omitted (out of scope — document ingestion is future work). They can be added in a later migration when document chunking is implemented.

## Rust Module Design

### Module: `nize_core::embedding`

```
crates/lib/nize_core/src/
├── embedding/
│   ├── mod.rs          # Public API: embed, embed_single, get_active_model, get_model_configs
│   ├── config.rs       # EmbeddingConfig resolution (env vars / config system)
│   ├── models.rs       # DB queries: EmbeddingModelConfig, get configs by provider
│   ├── provider.rs     # EmbeddingProvider trait + dispatch
│   ├── openai.rs       # OpenAI provider (reqwest → api.openai.com)
│   ├── ollama.rs       # Ollama provider (reqwest → localhost:11434)
│   └── local.rs        # Local deterministic FNV-based embedder
```

### Core Types

```rust
/// Registered embedding model from DB
pub struct EmbeddingModelConfig {
    pub provider: String,
    pub model: String,
    pub dimensions: i32,
    pub table_name: String,
    pub tool_table_name: String,
}

/// Result of embedding a single text
pub struct EmbeddingResult {
    pub text: String,
    pub embedding: Vec<f32>,
    pub model: String,
}

/// Resolved config for which provider/model to use
pub struct EmbeddingConfig {
    pub provider: String,
    pub active_model: String,
    pub ollama_base_url: String,
    pub openai_api_key: Option<String>,
}
```

### Public API (mirrors ref `embedder.ts`)

```rust
/// Embed multiple texts using ALL models for the active provider.
/// Returns one EmbeddingResult per text per model.
pub async fn embed(pool: &PgPool, config: &EmbeddingConfig, texts: &[String]) -> Result<Vec<EmbeddingResult>>;

/// Embed a single text using the active model only.
/// Returns the embedding vector.
pub async fn embed_single(pool: &PgPool, config: &EmbeddingConfig, text: &str) -> Result<Vec<f32>>;

/// Get all registered models for the active provider.
pub async fn get_model_configs(pool: &PgPool, provider: &str) -> Result<Vec<EmbeddingModelConfig>>;

/// Get the active model config.
pub async fn get_active_model(pool: &PgPool, config: &EmbeddingConfig) -> Result<EmbeddingModelConfig>;
```

### Provider Dispatch

```rust
/// Generate embeddings for a batch of texts using a specific model.
async fn embed_with_model(
    config: &EmbeddingConfig,
    texts: &[String],
    model_config: &EmbeddingModelConfig,
) -> Result<Vec<EmbeddingResult>>;
```

Dispatches based on `model_config.provider`:
- `"openai"` → `openai::embed_batch()` (with retry, max 3 attempts, exponential backoff)
- `"ollama"` → `ollama::embed_batch()` (sequential, one text at a time via `/api/embeddings`)
- `"local"` → `local::embed_batch()` (deterministic FNV hash, no network)

### Admin Config Definitions (seeded in migration)

Embedding settings are registered as admin-configurable values in the existing config system (`config_definitions` / `config_values` tables). This lets admins change provider/model at runtime via the settings UI without restarting.

| Config Key | Category | Type | Display Type | Default | Possible Values | Description |
|---|---|---|---|---|---|---|
| `embedding.provider` | `embedding` | `string` | `selector` | `ollama` | `["openai","ollama","local"]` | Embedding provider |
| `embedding.activeModel` | `embedding` | `string` | `text` | `nomic-embed-text` | — | Active embedding model name (must match a row in `embedding_models`) |
| `embedding.ollamaBaseUrl` | `embedding` | `string` | `text` | `http://localhost:11434` | — | Ollama API base URL |
| `embedding.openaiApiKey` | `embedding` | `string` | `text` | _(empty)_ | — | OpenAI API key (sensitive) |

These are seeded in the `0005_embedding_models.sql` migration using the same `INSERT ... ON CONFLICT DO UPDATE` pattern as existing config definitions (see migration 0003).

### Config Resolution

Priority order:
1. **Admin config system** — resolved via `config_values` (system scope) for `embedding.*` keys
2. **Environment variables** — `EMBEDDING_PROVIDER`, `EMBEDDING_ACTIVE_MODEL`, `OLLAMA_BASE_URL`, `OPENAI_API_KEY` used as fallback when no config value is set
3. **Definition defaults** — the `default_value` from `config_definitions` (last resort)

The `EmbeddingConfig::resolve()` function queries the config resolver for each key, falling back to env vars if the resolved value equals the definition default. This lets deployments use env vars for initial setup while admins can override via the UI.

If `OPENAI_API_KEY` is set (via env or config) and no explicit provider is configured, auto-select `"openai"`.

## Implementation Steps

### Step 1: Database Migration

Create `crates/lib/nize_core/migrations/0005_embedding_models.sql` with the schema above.

### Step 2: Embedding Config

Create `crates/lib/nize_core/src/embedding/config.rs`:
- `EmbeddingConfig` struct (provider, active_model, ollama_base_url, openai_api_key)
- `EmbeddingConfig::resolve(pool, cache)` — resolve from admin config system with env var fallback:
  1. Query `embedding.provider`, `embedding.activeModel`, `embedding.ollamaBaseUrl`, `embedding.openaiApiKey` via config resolver
  2. For each value, if the resolved value matches the definition default, check the corresponding env var (`EMBEDDING_PROVIDER`, `EMBEDDING_ACTIVE_MODEL`, `OLLAMA_BASE_URL`, `OPENAI_API_KEY`)
  3. Auto-select `"openai"` if `openai_api_key` is set and provider was not explicitly configured
- `EmbeddingConfig::from_env()` — simpler constructor for tests/CLI (env vars only, no DB)

### Step 3: DB Model Queries

Create `crates/lib/nize_core/src/embedding/models.rs`:
- `EmbeddingModelConfig` struct
- `get_model_configs(pool, provider)` — `SELECT` from `embedding_models WHERE provider = $1`
- `get_active_model(pool, config)` — find matching model by name

### Step 4: Local Provider

Create `crates/lib/nize_core/src/embedding/local.rs`:
- Deterministic FNV-1a hash-based embedding (port from ref `localEmbed()`)
- No external dependencies; useful for testing and offline mode

### Step 5: Ollama Provider

Create `crates/lib/nize_core/src/embedding/ollama.rs`:
- `POST {base_url}/api/embeddings` with `{ model, prompt }`
- Parse response `{ embedding: number[] }`
- Validate embedding length matches expected dimensions

### Step 6: OpenAI Provider

Create `crates/lib/nize_core/src/embedding/openai.rs`:
- `POST https://api.openai.com/v1/embeddings` with `{ model, input, dimensions }`
- Parse response `{ data: [{ embedding: number[] }] }`
- Retry logic: max 3 attempts, exponential backoff (2^attempt seconds)
- Requires `OPENAI_API_KEY`

### Step 7: Provider Dispatch + Public API

Create `crates/lib/nize_core/src/embedding/provider.rs` and `mod.rs`:
- `embed_with_model()` dispatch
- `embed()` — iterate models for provider, collect results
- `embed_single()` — embed one text with active model

### Step 8: Wire into nize_core

Update `crates/lib/nize_core/src/lib.rs`:
- Add `pub mod embedding;`

### Step 9: Tests

- **Unit tests** (no DB):
  - `local::embed()` produces deterministic output
  - Same text → same embedding (idempotent)
  - Different text → different embedding
  - Correct dimensions for given config
- **Integration tests** (with DB, `#[sqlx::test]`):
  - Migration runs successfully
  - `get_model_configs()` returns seeded models
  - `get_active_model()` finds the correct model
  - `embed_single()` with local provider returns correct dimensions

### Step 10: Build & Verify

1. `cargo build` — compilation succeeds
2. `cargo clippy` — no warnings
3. `cargo test -p nize_core` — all tests pass

## Dependencies

### Existing (already in workspace Cargo.toml)

- `sqlx` — DB queries for model registry
- `reqwest` — HTTP client for OpenAI and Ollama APIs
- `serde` / `serde_json` — JSON serialization for API requests/responses
- `tokio` — async runtime
- `log` / `tracing` — logging

### No New Dependencies Required

The pgvector `VECTOR` type is handled via raw SQL in migrations and queries (same pattern as the `vector` extension enabling already in `nize_core::db`). No Rust pgvector crate is needed since SQLx raw queries with `sql!` / `sqlx::query_as!` can handle vector columns as `Vec<f32>` or raw strings.

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Module in `nize_core` not `nize_mcp` | `nize_core` owns all domain logic and DB access; embeddings are a core capability used by both MCP tools and future document ingestion |
| `PgPool` passed explicitly | Follows existing `nize_core` pattern; no global state |
| `EmbeddingConfig` struct | Cleanly encapsulates resolved config; supports both admin config (DB) and env var fallback; testable |
| Local provider included | Enables testing and offline dev without Ollama/OpenAI; matches ref project |
| `Vec<f32>` for embeddings | Standard Rust float vector; pgvector stores as `real[]` compatible |
| Raw SQL for vector operations | SQLx compile-time checking doesn't natively support pgvector types; raw queries are the established pattern (see ref project's `sql.raw()`) |
| Tool embedding tables only | Chunk embedding tables deferred to document ingestion work |
| Default provider = `"ollama"` | Matches user's stated initial config; Ollama is local-first, no API key needed |
| Config via admin config system | Enables runtime changes via settings UI without restart; env vars as fallback for initial deployment; follows existing `config_definitions` / `config_values` pattern from migration 0003 |

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| PGlite may not support pgvector extension | PGlite 0.2+ includes pgvector; already validated by `enable_vector_extension()` in `nize_core::db` |
| Ollama not running locally | `local` provider as fallback; clear error messages when Ollama is unreachable |
| OpenAI rate limits | Exponential backoff retry (3 attempts); batch size limits can be added later |
| SQLx compile-time checks with vector columns | Use `sqlx::query!` for standard columns, raw `sqlx::query()` for vector operations |
| Migration ordering — `mcp_server_tools` FK | Migration 0005 runs after 0004 which creates `mcp_server_tools`; FK references are safe |

## Open Questions

None — the reference implementation is clear and the Rust port is straightforward.

## Completion Criteria

- [ ] Migration `0005_embedding_models.sql` creates `embedding_models` + 2 tool embedding tables
- [ ] `nize_core::embedding` module with `embed()`, `embed_single()`, `get_model_configs()`, `get_active_model()`
- [ ] Local provider returns deterministic embeddings with correct dimensions
- [ ] Ollama provider calls `/api/embeddings` and validates response dimensions
- [ ] OpenAI provider calls `/v1/embeddings` with retry logic
- [ ] Config resolves from env vars with sensible defaults
- [ ] Unit tests pass for local provider
- [ ] Integration tests pass for model config queries
- [ ] `cargo build`, `cargo clippy`, `cargo test -p nize_core` all green
