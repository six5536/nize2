import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import type { Plugin } from "vite";

// https://v2.tauri.app/start/frontend/vite/
const host = process.env.TAURI_DEV_HOST;

// Fixed dev ports — must match the debug defaults in nize_desktop/src/lib.rs.
const NIZE_WEB_PORT = 3100;
const NIZE_API_PORT = 3001;

// Vite plugin: serve /__nize-env.js so nize-web pages (loaded via proxy)
// can discover the API port and use relative URLs.
function nizeEnvPlugin(): Plugin {
  return {
    name: "nize-env",
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (req.url === "/__nize-env.js") {
          res.writeHead(200, {
            "Content-Type": "application/javascript",
            "Cache-Control": "no-cache",
          });
          res.end(`window.__NIZE_ENV__=${JSON.stringify({ apiPort: String(NIZE_API_PORT) })};\n`);
          return;
        }
        next();
      });
    },
  };
}

export default defineConfig(async () => ({
  plugins: [react(), nizeEnvPlugin()],

  // Vite options tailored for Tauri development
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
    // Same-origin proxy: nize-web and API sidecar appear under localhost:1420.
    proxy: {
      "/nize-web": {
        target: `http://127.0.0.1:${NIZE_WEB_PORT}`,
        ws: true,
      },
      "/auth": {
        target: `http://127.0.0.1:${NIZE_API_PORT}`,
      },
      "/config": {
        target: `http://127.0.0.1:${NIZE_API_PORT}`,
      },
      "/admin": {
        target: `http://127.0.0.1:${NIZE_API_PORT}`,
      },
      "/api": {
        target: `http://127.0.0.1:${NIZE_API_PORT}`,
      },
    },
  },
}));
