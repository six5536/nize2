-- Rename config key from mcp.max_stdio_processes to mcp.max_managed_processes
-- to reflect that the limit now applies to all managed transports (stdio, managed-sse, managed-http).

UPDATE config_definitions
SET key = 'mcp.max_managed_processes',
    label = 'Max Managed Processes',
    description = 'Maximum number of concurrent managed MCP server processes (default: 50)',
    validators = '[{"type":"min","value":1,"message":"Must allow at least 1 managed process"},{"type":"max","value":500,"message":"Cannot exceed 500 managed processes"}]'::jsonb
WHERE key = 'mcp.max_stdio_processes';

-- Also update any user overrides that reference the old key
UPDATE config_values
SET key = 'mcp.max_managed_processes'
WHERE key = 'mcp.max_stdio_processes';
