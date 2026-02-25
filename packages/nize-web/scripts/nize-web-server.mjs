// @awa-component: PLAN-012-NizeWebServer
// @awa-impl: PLAN-021 — simplified: no dev mode, no reverse proxy
// nize-web production sidecar wrapper.
//
// Starts the Next.js standalone server on a port and prints
// {"port": N} to stdout once the server is ready (matches the sidecar protocol).
//
// Serves /__nize-env.js from memory so the frontend can discover the API port
// without touching the filesystem.
//
// Usage:
//   bun nize-web-server.mjs --port=<N>
//   bun nize-web-server.mjs --port=<N> --api-port=<M>
//   bun nize-web-server.mjs --port=<N> --api-port=<M> --mcp-port=<P>

import { parseArgs } from "node:util";
import { createServer } from "node:net";
import { spawn, execSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const isBun = typeof globalThis.Bun !== "undefined";

const __dirname = dirname(fileURLToPath(import.meta.url));

const { values: args } = parseArgs({
  options: {
    port: { type: "string", default: "0" },
    "api-port": { type: "string", default: "" },
    "mcp-port": { type: "string", default: "" },
  },
});

const requestedPort = parseInt(args.port, 10);
const apiPort = args["api-port"];
const mcpPort = args["mcp-port"];

// @awa-impl: PLAN-012-2.1 — find free port for ephemeral binding
async function findFreePort() {
  return new Promise((resolve, reject) => {
    const srv = createServer();
    srv.listen(0, "127.0.0.1", () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
    srv.on("error", reject);
  });
}

const port = requestedPort === 0 ? await findFreePort() : requestedPort;

// Kill any stale process left on the requested port from a previous run
// (e.g. after an unclean shutdown of cargo tauri dev).
if (requestedPort !== 0) {
  try {
    const pids = execSync(`lsof -ti tcp:${port}`, { encoding: "utf8" }).trim();
    if (pids) {
      for (const pid of pids.split("\n")) {
        process.stderr.write(`nize-web: killing stale process ${pid} on port ${port}\n`);
        try {
          process.kill(parseInt(pid, 10), "SIGKILL");
        } catch {
          /* already gone */
        }
      }
      // Brief pause so the OS reclaims the port.
      await new Promise((r) => setTimeout(r, 300));
    }
  } catch {
    /* lsof returns non-zero when no matches — that's fine */
  }
}

// Prepare runtime env payload served at /__nize-env.js.
const envPayload = `window.__NIZE_ENV__=${JSON.stringify({ apiPort: apiPort || "" })};\n`;

// @awa-impl: PLAN-012-2.1 — start Next.js standalone server
// In a monorepo the standalone output nests the server under packages/nize-web/.
const standaloneDir = join(__dirname, "standalone");
const serverPath = join(standaloneDir, "packages", "nize-web", "server.js");

// Start Next.js on an internal port, proxy through a front port to
// inject /__nize-env.js and forward API routes without touching the filesystem.
const internalPort = await findFreePort();

const child = spawn(process.execPath, [serverPath], {
  cwd: standaloneDir,
  env: {
    ...process.env,
    PORT: String(internalPort),
    HOSTNAME: "127.0.0.1",
    // @awa-impl: PLAN-029-2.2 — pass MCP port to Next.js process for nize-chat
    ...(mcpPort ? { NIZE_MCP_PORT: mcpPort } : {}),
  },
  stdio: ["pipe", "pipe", "inherit"],
});

// Forward child stdout to stderr so it doesn't interfere with the sidecar protocol.
child.stdout.pipe(process.stderr);

// @awa-impl: PLAN-012-2.1 — poll until server is ready
async function waitForServer(targetPort, maxAttempts = 50) {
  for (let i = 0; i < maxAttempts; i++) {
    try {
      const response = await fetch(`http://127.0.0.1:${targetPort}/`);
      if (response.ok || response.status < 500) return;
    } catch {
      // Not ready yet
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`nize-web server did not start within ${maxAttempts * 200}ms`);
}

await waitForServer(internalPort);

// API path prefix — requests under /api are forwarded to the API server
// (when --api-port is provided), keeping cookies first-party.
const apiPrefixes = ["/api"];

function proxyTarget(url) {
  const path = url.split("?")[0];
  if (apiPort && apiPrefixes.some((p) => path === p || path.startsWith(p + "/"))) {
    return parseInt(apiPort, 10);
  }
  return internalPort;
}

// Lightweight reverse proxy: serves /__nize-env.js from memory,
// forwards API routes to the API server (when available),
// proxies all other requests to the Next.js standalone server.

if (isBun) {
  const proxy = Bun.serve({
    port,
    hostname: "127.0.0.1",

    async fetch(req) {
      const url = new URL(req.url);

      // Serve /__nize-env.js from memory
      if (url.pathname === "/__nize-env.js") {
        return new Response(envPayload, {
          headers: { "Content-Type": "application/javascript", "Cache-Control": "no-cache" },
        });
      }

      // Reverse proxy: forward to API or Next.js
      const target = proxyTarget(url.pathname);
      const proxyUrl = `http://127.0.0.1:${target}${url.pathname}${url.search}`;
      const proxyHeaders = new Headers(req.headers);
      proxyHeaders.delete("accept-encoding");
      const proxyReq = new Request(proxyUrl, {
        method: req.method,
        headers: proxyHeaders,
        body: req.body,
        redirect: "manual",
      });
      try {
        const upstream = await fetch(proxyReq);
        const respHeaders = new Headers(upstream.headers);
        respHeaders.delete("content-encoding");
        respHeaders.delete("content-length");
        return new Response(upstream.body, {
          status: upstream.status,
          statusText: upstream.statusText,
          headers: respHeaders,
        });
      } catch {
        return new Response("Bad Gateway", { status: 502 });
      }
    },
  });
} else {
  // Node.js path: http.createServer
  const http = await import("node:http");

  const proxy = http.createServer((req, res) => {
    if (req.url === "/__nize-env.js") {
      res.writeHead(200, {
        "Content-Type": "application/javascript",
        "Cache-Control": "no-cache",
      });
      res.end(envPayload);
      return;
    }

    const target = proxyTarget(req.url);
    const proxyReq = http.request(
      {
        hostname: "127.0.0.1",
        port: target,
        path: req.url,
        method: req.method,
        headers: req.headers,
      },
      (proxyRes) => {
        res.writeHead(proxyRes.statusCode, proxyRes.headers);
        proxyRes.pipe(res, { end: true });
      },
    );

    proxyReq.on("error", (err) => {
      res.writeHead(502);
      res.end("Bad Gateway");
    });

    req.pipe(proxyReq, { end: true });
  });

  proxy.listen(port, "127.0.0.1");
}

// @awa-impl: PLAN-012-2.1 — print JSON port to stdout (sidecar protocol)
process.stdout.write(JSON.stringify({ port }) + "\n");

// @awa-impl: PLAN-012-2.1 — graceful shutdown
function shutdown() {
  child.kill("SIGTERM");
  setTimeout(() => process.exit(0), 5000);
}

child.on("exit", (code) => process.exit(code ?? 0));
process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);

// Detect parent death: when the parent process exits (even via SIGKILL), our
// stdin pipe is closed.  Trigger the same graceful shutdown so `next dev` is
// killed and the .next/dev/lock file is released.
process.stdin.resume();
process.stdin.on("end", shutdown);
