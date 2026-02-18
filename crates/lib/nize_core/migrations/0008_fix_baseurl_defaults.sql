-- Fix base URL config defaults: change from provider-specific URLs to empty
-- strings. Empty means "use SDK built-in default"; non-empty is for custom
-- overrides (e.g., Azure OpenAI, corporate proxies).
--
-- The previous defaults (e.g., "https://api.anthropic.com") were always
-- returned by the config system and passed to the AI SDK, overriding the
-- SDK's own correct defaults (e.g., "https://api.anthropic.com/v1").

UPDATE config_definitions
SET default_value = '',
    description = 'Custom base URL for Anthropic API requests (leave empty for default)'
WHERE key = 'agent.baseUrl.anthropic'
  AND default_value != '';

UPDATE config_definitions
SET default_value = '',
    description = 'Custom base URL for OpenAI API requests (leave empty for default)'
WHERE key = 'agent.baseUrl.openai'
  AND default_value != '';

UPDATE config_definitions
SET default_value = '',
    description = 'Custom base URL for Google AI API requests (leave empty for default)'
WHERE key = 'agent.baseUrl.google'
  AND default_value != '';
