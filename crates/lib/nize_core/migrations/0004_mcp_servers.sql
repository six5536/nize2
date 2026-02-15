-- MCP server registry tables
-- Ported from ref project: packages/db/src/schema/mcp.ts

-- Enums
DO $$ BEGIN
    CREATE TYPE visibility_tier AS ENUM ('hidden', 'visible', 'user');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE transport_type AS ENUM ('stdio', 'http');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE auth_type AS ENUM ('none', 'api-key', 'oauth');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- ---------------------------------------------------------------------------
-- mcp_servers: MCP server registrations
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS mcp_servers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    domain TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    visibility visibility_tier NOT NULL DEFAULT 'visible',
    transport transport_type NOT NULL DEFAULT 'http',
    config JSONB,
    oauth_config JSONB,
    default_response_size_limit INTEGER,
    owner_id UUID REFERENCES users(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT true,
    available BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS mcp_servers_domain_idx ON mcp_servers (domain);
CREATE INDEX IF NOT EXISTS mcp_servers_enabled_idx ON mcp_servers (enabled);
CREATE INDEX IF NOT EXISTS mcp_servers_visibility_idx ON mcp_servers (visibility);
CREATE INDEX IF NOT EXISTS mcp_servers_owner_idx ON mcp_servers (owner_id);

-- ---------------------------------------------------------------------------
-- mcp_server_tools: Tool manifests from registered servers
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS mcp_server_tools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    manifest JSONB NOT NULL,
    response_size_limit INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS mcp_server_tools_server_idx ON mcp_server_tools (server_id);
CREATE UNIQUE INDEX IF NOT EXISTS mcp_server_tools_server_name_idx ON mcp_server_tools (server_id, name);

-- ---------------------------------------------------------------------------
-- user_mcp_preferences: Per-user MCP server enablement
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS user_mcp_preferences (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, server_id)
);

CREATE INDEX IF NOT EXISTS user_mcp_preferences_user_idx ON user_mcp_preferences (user_id);

-- ---------------------------------------------------------------------------
-- mcp_server_secrets: Encrypted secrets for MCP servers
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS mcp_server_secrets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    server_id UUID NOT NULL UNIQUE REFERENCES mcp_servers(id) ON DELETE CASCADE,
    api_key_encrypted TEXT,
    oauth_client_secret_encrypted TEXT,
    encryption_key_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- mcp_oauth_tokens: Per-user OAuth tokens for MCP servers
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS mcp_oauth_tokens (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    access_token_encrypted TEXT NOT NULL,
    refresh_token_encrypted TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    scopes TEXT[] NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, server_id)
);

CREATE INDEX IF NOT EXISTS mcp_oauth_tokens_user_idx ON mcp_oauth_tokens (user_id);
CREATE INDEX IF NOT EXISTS mcp_oauth_tokens_server_idx ON mcp_oauth_tokens (server_id);
CREATE INDEX IF NOT EXISTS mcp_oauth_tokens_expires_idx ON mcp_oauth_tokens (expires_at);

-- ---------------------------------------------------------------------------
-- mcp_config_audit: Audit log for configuration changes
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS mcp_config_audit (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID NOT NULL REFERENCES users(id) ON DELETE SET NULL,
    server_id UUID,
    server_name TEXT NOT NULL,
    action TEXT NOT NULL,
    details JSONB,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS mcp_config_audit_actor_idx ON mcp_config_audit (actor_id);
CREATE INDEX IF NOT EXISTS mcp_config_audit_server_idx ON mcp_config_audit (server_id);
CREATE INDEX IF NOT EXISTS mcp_config_audit_action_idx ON mcp_config_audit (action);
CREATE INDEX IF NOT EXISTS mcp_config_audit_created_idx ON mcp_config_audit (created_at);
