// @zen-component: PLAN-012-NizeWebServer
// nize-web sidecar wrapper.
//
// Starts the Next.js standalone server on an ephemeral port and prints
// {"port": N} to stdout once the server is ready (matches the sidecar protocol).
//
// Usage:
//   node nize-web-server.mjs --port=<N>

import { parseArgs } from "node:util";
import { createServer } from "node:net";
import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import http from "node:http";

const __dirname = dirname(fileURLToPath(import.meta.url));

const { values: args } = parseArgs({
  options: {
    port: { type: "string", default: "0" },
    "api-port": { type: "string", default: "" },
  },
});

const requestedPort = parseInt(args.port, 10);
const apiPort = args["api-port"];

// @zen-impl: PLAN-012-2.1 — find free port for ephemeral binding
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

// @zen-impl: PLAN-012-2.1 — start Next.js standalone server
// In a monorepo the standalone output nests the server under packages/nize-web/.
const standaloneDir = join(__dirname, "standalone");
const serverPath = join(standaloneDir, "packages", "nize-web", "server.js");

// Prepare runtime env payload served at /__nize-env.js.
// We serve this from a lightweight proxy instead of writing into the
// resources tree, which would trigger Tauri's file watcher in dev mode.
const envPayload = `window.__NIZE_ENV__=${JSON.stringify({ apiPort: apiPort || "" })};\n`;

// Start Next.js on an internal port, then proxy through a front port
// that injects /__nize-env.js without touching the filesystem.
const internalPort = await findFreePort();

const child = spawn(process.execPath, [serverPath], {
  cwd: standaloneDir,
  env: {
    ...process.env,
    PORT: String(internalPort),
    HOSTNAME: "127.0.0.1",
  },
  stdio: ["pipe", "pipe", "inherit"],
});

// Forward child stdout to stderr so it doesn't interfere with the sidecar protocol.
child.stdout.pipe(process.stderr);

// @zen-impl: PLAN-012-2.1 — poll until server is ready
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

// API path prefixes — requests matching these are forwarded to the API server
// (when --api-port is provided), keeping cookies first-party.
const apiPrefixes = ["/auth/", "/config/", "/admin/", "/api/"];

function isApiPath(url) {
  const path = url.split("?")[0];
  return apiPort && apiPrefixes.some((p) => path.startsWith(p));
}

// Lightweight reverse proxy: serves /__nize-env.js from memory,
// forwards API routes to the API server (when available),
// proxies all other requests to the Next.js standalone server.
const proxy = http.createServer((req, res) => {
  if (req.url === "/__nize-env.js") {
    res.writeHead(200, {
      "Content-Type": "application/javascript",
      "Cache-Control": "no-cache",
    });
    res.end(envPayload);
    return;
  }

  // Forward API paths to the API server
  if (isApiPath(req.url)) {
    const proxyReq = http.request(
      {
        hostname: "127.0.0.1",
        port: parseInt(apiPort, 10),
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
    return;
  }

  const proxyReq = http.request(
    {
      hostname: "127.0.0.1",
      port: internalPort,
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

// @zen-impl: PLAN-012-2.1 — print JSON port to stdout (sidecar protocol)
process.stdout.write(JSON.stringify({ port }) + "\n");

// @zen-impl: PLAN-012-2.1 — graceful shutdown
function shutdown() {
  child.kill("SIGTERM");
  setTimeout(() => process.exit(0), 5000);
}

child.on("exit", (code) => process.exit(code ?? 0));
process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
