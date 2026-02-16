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
