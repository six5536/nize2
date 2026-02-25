-- @awa-impl: PLAN-029-4.1 — tool calling config definitions
-- Add config definitions for MCP tool calling settings.

-- agent.tools.enabled — toggle for MCP tool calling
INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'agent.tools.enabled',
    'agent',
    'boolean',
    'boolean',
    'true',
    'Enable MCP Tool Calling',
    'Allow the AI to discover and execute external MCP tools during chat'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;

-- agent.tools.maxSteps — max tool-call steps per message
INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description, validators)
VALUES (
    'agent.tools.maxSteps',
    'agent',
    'number',
    'number',
    '10',
    'Max Tool Steps',
    'Maximum number of tool-call steps the AI can take per message',
    '[{"type":"min","value":1,"message":"Max steps must be at least 1"},{"type":"max","value":50,"message":"Max steps must be at most 50"}]'::jsonb
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    validators = EXCLUDED.validators;

-- agent.tools.systemPrompt — system prompt injected when tools are enabled
INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description)
VALUES (
    'agent.tools.systemPrompt',
    'agent',
    'string',
    'longText',
    'You have access to tools for discovering and executing external MCP tools. Use `discover_tools` to find relevant tools, `get_tool_schema` to understand parameters, and `execute_tool` to run them. Use `list_tool_domains` and `browse_tool_domain` to explore available categories.',
    'Tools System Prompt',
    'System prompt prepended to guide the AI when tool calling is enabled'
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description;
