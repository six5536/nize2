-- Configuration system tables and seed data
-- Two-table model: definitions (metadata) + values (runtime data)

-- Scope enum for config values
DO $$ BEGIN
    CREATE TYPE config_scope AS ENUM ('system', 'user-override');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Config definitions — seed data defining available config keys
CREATE TABLE IF NOT EXISTS config_definitions (
    key VARCHAR(255) PRIMARY KEY,
    category VARCHAR(100) NOT NULL,
    type VARCHAR(50) NOT NULL,
    display_type VARCHAR(50) NOT NULL,
    possible_values JSONB,
    validators JSONB,
    default_value TEXT NOT NULL,
    label VARCHAR(255),
    description TEXT
);

CREATE INDEX IF NOT EXISTS config_def_category_idx ON config_definitions (category);

-- Config values — runtime data storing actual values at each scope
CREATE TABLE IF NOT EXISTS config_values (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key VARCHAR(255) NOT NULL REFERENCES config_definitions(key) ON DELETE CASCADE,
    scope config_scope NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS config_val_key_scope_user_idx ON config_values (key, scope, user_id);
CREATE INDEX IF NOT EXISTS config_val_scope_idx ON config_values (scope);
CREATE INDEX IF NOT EXISTS config_val_user_idx ON config_values (user_id);

-- ---------------------------------------------------------------------------
-- Seed config definitions (idempotent via ON CONFLICT DO UPDATE)
-- ---------------------------------------------------------------------------

-- System Settings — Cache TTLs
INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description, validators)
VALUES (
    'system.cache.ttlSystem',
    'system',
    'number',
    'number',
    '300000',
    'System Cache TTL',
    'Cache TTL for system config in milliseconds (default: 5 minutes)',
    '[{"type":"min","value":0,"message":"TTL must be non-negative"}]'::jsonb
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
    'system.cache.ttlUserOverride',
    'system',
    'number',
    'number',
    '30000',
    'User Override Cache TTL',
    'Cache TTL for user-override config in milliseconds (default: 30 seconds)',
    '[{"type":"min","value":0,"message":"TTL must be non-negative"}]'::jsonb
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    validators = EXCLUDED.validators;

-- Agent Settings
INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description, validators)
VALUES (
    'agent.model.temperature',
    'agent',
    'number',
    'number',
    '0.7',
    'Temperature',
    'Controls randomness in AI responses (0 = deterministic, 2 = creative)',
    '[{"type":"min","value":0,"message":"Temperature must be at least 0"},{"type":"max","value":2,"message":"Temperature must be at most 2"}]'::jsonb
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
    'agent.model.name',
    'agent',
    'string',
    'text',
    'anthropic:claude-haiku-4-5-20251001',
    'Model Name',
    'The AI model to use for chat (format: provider:model)',
    '[{"type":"required","message":"Model name is required"}]'::jsonb
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
    'agent.context.maxResults',
    'agent',
    'number',
    'number',
    '5',
    'Max Context Results',
    'Maximum number of search results to include in context',
    '[{"type":"min","value":1,"message":"Max results must be at least 1"},{"type":"max","value":20,"message":"Max results must be at most 20"}]'::jsonb
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
    'agent.compaction.maxMessages',
    'agent',
    'number',
    'number',
    '20',
    'Compaction Max Messages',
    'Maximum messages before context compaction triggers',
    '[{"type":"min","value":5,"message":"Max messages must be at least 5"},{"type":"max","value":100,"message":"Max messages must be at most 100"}]'::jsonb
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    validators = EXCLUDED.validators;

-- UI Settings
INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description, possible_values, validators)
VALUES (
    'ui.theme',
    'ui',
    'string',
    'selector',
    'auto',
    'Theme',
    'Application color theme',
    '["light","dark","auto"]'::jsonb,
    '[{"type":"required","message":"Theme is required"}]'::jsonb
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
