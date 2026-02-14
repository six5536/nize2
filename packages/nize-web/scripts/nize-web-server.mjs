// @zen-component: PLAN-012-NizeWebServer
// nize-web sidecar wrapper.
//
// Starts the Next.js standalone server on an ephemeral port and prints
// {"port": N} to stdout once the server is ready (matches the sidecar protocol).
//
// Usage:
//   node nize-web-server.mjs --port=<N>
//   node nize-web-server.mjs --port=<N> --dev          (hot-reload via next dev)

import { parseArgs } from "node:util";
import { createServer } from "node:net";
import { spawn, execSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { unlinkSync } from "node:fs";
import http from "node:http";

const __dirname = dirname(fileURLToPath(import.meta.url));

const { values: args } = parseArgs({
  options: {
    port: { type: "string", default: "0" },
    "api-port": { type: "string", default: "" },
    dev: { type: "boolean", default: false },
  },
});

const requestedPort = parseInt(args.port, 10);
const apiPort = args["api-port"];
const devMode = args.dev;

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

// Kill any stale process left on the requested port from a previous run
// (e.g. after an unclean shutdown of cargo tauri dev).
if (requestedPort !== 0) {
  try {
    const pids = execSync(`lsof -ti tcp:${port}`, { encoding: "utf8" }).trim();
    if (pids) {
      for (const pid of pids.split("\n")) {
        process.stderr.write(`nize-web: killing stale process ${pid} on port ${port}\n`);
        try { process.kill(parseInt(pid, 10), "SIGKILL"); } catch { /* already gone */ }
      }
      // Brief pause so the OS reclaims the port.
      await new Promise((r) => setTimeout(r, 300));
    }
  } catch { /* lsof returns non-zero when no matches — that's fine */ }
}

// Remove stale Next.js dev lock file from a previous unclean shutdown.
try {
  unlinkSync(join(__dirname, "..", ".next", "dev", "lock"));
} catch { /* doesn't exist — fine */ }

// Prepare runtime env payload served at /__nize-env.js.
// We serve this from a lightweight proxy instead of writing into the
// resources tree, which would trigger Tauri's file watcher in dev mode.
const envPayload = `window.__NIZE_ENV__=${JSON.stringify({ apiPort: apiPort || "" })};\n`;

// Start Next.js on an internal port, then proxy through a front port
// that injects /__nize-env.js without touching the filesystem.
const internalPort = await findFreePort();

// @zen-impl: PLAN-014-1 — spawn next dev (HMR) or standalone server
let child;
if (devMode) {
  // Dev mode: run `next dev` for hot-module-reload.
  // Resolve the nize-web package root (one level up from scripts/).
  const nizeWebRoot = join(__dirname, "..");
  child = spawn("npx", ["next", "dev", "--port", String(internalPort)], {
    cwd: nizeWebRoot,
    env: {
      ...process.env,
      HOSTNAME: "127.0.0.1",
      NIZE_WEB_BASE_PATH: "/nize-web",
    },
    stdio: ["pipe", "pipe", "inherit"],
  });
} else {
  // @zen-impl: PLAN-012-2.1 — start Next.js standalone server
  // In a monorepo the standalone output nests the server under packages/nize-web/.
  const standaloneDir = join(__dirname, "standalone");
  const serverPath = join(standaloneDir, "packages", "nize-web", "server.js");
  child = spawn(process.execPath, [serverPath], {
    cwd: standaloneDir,
    env: {
      ...process.env,
      PORT: String(internalPort),
      HOSTNAME: "127.0.0.1",
      NIZE_WEB_BASE_PATH: "/nize-web",
    },
    stdio: ["pipe", "pipe", "inherit"],
  });
}

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
  if (req.url === "/__nize-env.js" || req.url === "/nize-web/__nize-env.js") {
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

// @zen-impl: PLAN-014-1 — proxy WebSocket upgrades for Next.js HMR
proxy.on("upgrade", (req, socket, head) => {
  const proxyReq = http.request({
    hostname: "127.0.0.1",
    port: internalPort,
    path: req.url,
    method: req.method,
    headers: req.headers,
  });

  proxyReq.on("upgrade", (proxyRes, proxySocket, proxyHead) => {
    socket.write(
      `HTTP/${proxyRes.httpVersion} ${proxyRes.statusCode} ${proxyRes.statusMessage}\r\n` +
        Object.entries(proxyRes.headers)
          .map(([k, v]) => `${k}: ${v}`)
          .join("\r\n") +
        "\r\n\r\n",
    );
    if (proxyHead.length) socket.write(proxyHead);
    proxySocket.pipe(socket);
    socket.pipe(proxySocket);
  });

  proxyReq.on("error", () => socket.end());
  proxyReq.end();
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

// Detect parent death: when the parent process exits (even via SIGKILL), our
// stdin pipe is closed.  Trigger the same graceful shutdown so `next dev` is
// killed and the .next/dev/lock file is released.
process.stdin.resume();
process.stdin.on("end", shutdown);
