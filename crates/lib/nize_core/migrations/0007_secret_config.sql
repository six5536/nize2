-- Secret config: API key definitions with display_type = 'secret'
-- and base URL definitions for AI providers.
-- Also migrates embedding.openaiApiKey → embedding.apiKey.openai.

-- ---------------------------------------------------------------------------
-- Agent API key definitions (secret — encrypted at rest)
-- ---------------------------------------------------------------------------

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'agent.apiKey.anthropic',
    'agent',
    'string',
    'secret',
    '',
    'Anthropic API Key',
    'API key for Anthropic models (Claude)'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'agent.apiKey.openai',
    'agent',
    'string',
    'secret',
    '',
    'OpenAI API Key',
    'API key for OpenAI models (GPT)'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'agent.apiKey.google',
    'agent',
    'string',
    'secret',
    '',
    'Google API Key',
    'API key for Google AI models (Gemini)'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;

-- ---------------------------------------------------------------------------
-- Agent base URL definitions (text — user-configurable)
-- ---------------------------------------------------------------------------

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'agent.baseUrl.anthropic',
    'agent',
    'string',
    'text',
    'https://api.anthropic.com',
    'Anthropic Base URL',
    'Base URL for Anthropic API requests'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'agent.baseUrl.openai',
    'agent',
    'string',
    'text',
    'https://api.openai.com/v1',
    'OpenAI Base URL',
    'Base URL for OpenAI API requests'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'agent.baseUrl.google',
    'agent',
    'string',
    'text',
    'https://generativelanguage.googleapis.com',
    'Google AI Base URL',
    'Base URL for Google AI API requests'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;

-- ---------------------------------------------------------------------------
-- Embedding API key: rename embedding.openaiApiKey → embedding.apiKey.openai
-- ---------------------------------------------------------------------------

-- Create new definition
INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'embedding.apiKey.openai',
    'embedding',
    'string',
    'secret',
    '',
    'OpenAI API Key (Embeddings)',
    'API key for OpenAI embeddings (required when provider is openai)'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;

-- Copy existing values from old key to new key (plaintext → kept as-is;
-- the application layer will encrypt on next write).
-- NOTE: This is pre-alpha; DB can be cleared. Values are copied verbatim.
INSERT INTO config_values (key, scope, user_id, value, updated_at)
SELECT 'embedding.apiKey.openai', scope, user_id, value, now()
FROM config_values
WHERE key = 'embedding.openaiApiKey'
ON CONFLICT (key, scope, user_id) DO UPDATE SET
    value = EXCLUDED.value,
    updated_at = EXCLUDED.updated_at;

-- Drop old values (cascade from definition delete won't work here since
-- we're keeping the definition briefly; delete values first)
DELETE FROM config_values WHERE key = 'embedding.openaiApiKey';

-- Drop old definition
DELETE FROM config_definitions WHERE key = 'embedding.openaiApiKey';
