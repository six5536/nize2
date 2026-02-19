-- Add id_token_encrypted column to mcp_oauth_tokens for Google OIDC id_token storage
ALTER TABLE mcp_oauth_tokens ADD COLUMN IF NOT EXISTS id_token_encrypted TEXT;
