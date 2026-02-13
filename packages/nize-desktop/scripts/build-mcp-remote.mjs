// @zen-impl: PLAN-011-1.1 — esbuild config for mcp-remote bundle
import * as esbuild from "esbuild";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const outDir = join(__dirname, "..", "..", "..", "crates", "app", "nize_desktop", "resources", "mcp-remote");

mkdirSync(outDir, { recursive: true });

// Bundle mcp-remote proxy (stdio ↔ HTTP Streamable bridge) into a single ESM file.
// The entry point is dist/proxy.js from the mcp-remote package (the "mcp-remote" bin).
// Use import.meta.resolve to handle npm workspace hoisting.
const mcpRemoteEntry = fileURLToPath(import.meta.resolve("mcp-remote/dist/proxy.js"));

await esbuild.build({
  entryPoints: [mcpRemoteEntry],
  bundle: true,
  format: "esm",
  platform: "node",
  target: "node22",
  outfile: join(outDir, "mcp-remote.mjs"),
  // Mark node builtins as external.
  external: ["node:*"],
  banner: {
    js: ["// @zen-impl: PLAN-011-1.1 — bundled mcp-remote stdio proxy", "import { createRequire as __createRequire } from 'node:module';", "const require = __createRequire(import.meta.url);"].join("\n"),
  },
});

console.log(`mcp-remote bundled to ${outDir}`);
