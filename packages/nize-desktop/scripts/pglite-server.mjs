// @awa-component: PLAN-007-PgLiteServer
// PGlite server entry point.
//
// Starts a PGlite instance with pgvector and exposes the standard PG wire
// protocol via pglite-socket. Prints {"port": N} to stdout once listening
// (matches the nize_desktop_server sidecar protocol).
//
// Usage:
//   node pglite-server.mjs --db=<path> --port=<N> --database=<name>

import { PGlite } from "@electric-sql/pglite";
import { PGLiteSocketServer } from "@electric-sql/pglite-socket";

// Define the vector extension inline so that the bundle path resolves
// relative to this file (./vector.tar.gz) rather than the library's
// internal ../vector.tar.gz which breaks after esbuild bundling.
const vector = {
  name: "pgvector",
  setup: async (_pg, emscriptenOpts) => ({
    emscriptenOpts,
    bundlePath: new URL("./vector.tar.gz", import.meta.url),
  }),
};
import { parseArgs } from "node:util";

const { values: args } = parseArgs({
  options: {
    db: { type: "string", default: "./pgdata" },
    port: { type: "string", default: "0" },
    database: { type: "string", default: "nize" },
  },
});

const dataDir = args.db;
const requestedPort = parseInt(args.port, 10);
const databaseName = args.database;

// @awa-impl: PLAN-007-1.2 — create PGlite instance with vector extension
const db = new PGlite({
  dataDir: `file://${dataDir}`,
  extensions: { vector },
});

await db.waitReady;

// @awa-impl: PLAN-007-1.2 — enable pgvector extension
await db.exec("CREATE EXTENSION IF NOT EXISTS vector");

// @awa-impl: PLAN-007-1.2 — create application database (PGlite runs in single-db mode)
// PGlite doesn't support CREATE DATABASE — it operates on a single database.
// The database name argument is informational only.

// @awa-impl: PLAN-007-1.2 — start pglite-socket server
const server = new PGLiteSocketServer({
  db,
  port: requestedPort,
  host: "127.0.0.1",
});

// @awa-impl: PLAN-007-1.2 — print JSON port to stdout (sidecar protocol)
server.addEventListener("listening", (event) => {
  const { port } = event.detail;
  const ready = JSON.stringify({ port });
  process.stdout.write(ready + "\n");
});

await server.start();

// @awa-impl: PLAN-007-1.2 — graceful shutdown
async function shutdown() {
  try {
    await server.stop();
    await db.close();
  } catch {
    // Ignore errors during shutdown.
  }
  process.exit(0);
}

process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
