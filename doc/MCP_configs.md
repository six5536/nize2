
```json
{
  "mcpServers": {
    "filesystem": {
      "args": [
        "-y",
        "@modelcontextprotocol/server-filesystem",
        "/Users/rich/Desktop"
      ],
      "command": "npx",
      "env": {
        "PATH": "/Users/rich/.local/share/mise/installs/node/24.3.0/bin:/usr/local/bin:/usr/bin:/bin"
      }
    },
    "nize": {
      "args": ["/Users/rich/dev/six5536/git/nize-mcp/crates/app/nize_desktop/resources/mcp-remote/mcp-remote.mjs", "http://127.0.0.1:19560/mcp", "--allow-http", "--header", "Authorization:${AUTH_TOKEN}"],
      "command": "bun",
      "env": {
        "AUTH_TOKEN": "Bearer iG0XpLAs0SMBkflQBImPBXtB5UfmHHRxmIbIr7Uv2hJraVEahUwoctdtGSOfyLbP",
        "PATH": "/Users/rich/dev/six5536/git/nize-mcp/target/debug:/usr/local/bin:/usr/bin:/bin"
      }
    }
  },
  "preferences": {
    "coworkScheduledTasksEnabled": false,
    "sidebarMode": "chat"
  }
}


```