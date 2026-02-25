// @awa-impl: PLAN-007-1.3 — esbuild config for pglite-server bundle
import * as esbuild from "esbuild";
import { copyFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const outDir = join(__dirname, "..", "..", "..", "crates", "app", "nize_desktop", "resources", "pglite");

mkdirSync(outDir, { recursive: true });

// Bundle pglite-server.mjs into a single ESM file.
await esbuild.build({
  entryPoints: [join(__dirname, "pglite-server.mjs")],
  bundle: true,
  format: "esm",
  platform: "node",
  target: "node24",
  outfile: join(outDir, "pglite-server.mjs"),
  // PGlite loads WASM files from the filesystem at runtime.
  // Mark node builtins as external.
  external: ["node:*"],
  banner: {
    js: "// @awa-impl: PLAN-007-1.3 — bundled PGlite server",
  },
});

// Copy PGlite WASM files and extension bundles to the output directory.
// Resolve from node_modules directly (package.json subpath not exported in v0.3.x).
const pgliteDist = join(__dirname, "..", "node_modules", "@electric-sql", "pglite", "dist");
const assets = ["pglite.wasm", "pglite.data", "vector.tar.gz"];
for (const file of assets) {
  const src = join(pgliteDist, file);
  if (existsSync(src)) {
    copyFileSync(src, join(outDir, file));
    console.log(`Copied ${file}`);
  }
}

console.log(`PGlite server bundled to ${outDir}`);
