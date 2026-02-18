-- MCP stdio server configuration: max concurrent stdio processes

INSERT INTO config_definitions (key, category, type, display_type, default_value, label, description, validators)
VALUES (
    'mcp.max_stdio_processes',
    'mcp',
    'number',
    'number',
    '50',
    'Max Stdio Processes',
    'Maximum number of concurrent stdio MCP server processes (default: 50)',
    '[{"type":"min","value":1,"message":"Must allow at least 1 stdio process"},{"type":"max","value":500,"message":"Cannot exceed 500 stdio processes"}]'::jsonb
)
ON CONFLICT (key) DO UPDATE SET
    category = EXCLUDED.category,
    type = EXCLUDED.type,
    display_type = EXCLUDED.display_type,
    default_value = EXCLUDED.default_value,
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    validators = EXCLUDED.validators;
