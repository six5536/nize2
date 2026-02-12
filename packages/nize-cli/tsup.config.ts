import { defineConfig } from "tsup";

export default defineConfig([
  // Node builds (ESM / CJS)
  {
    entry: ["src/index.ts"],
    format: ["esm", "cjs"],
    target: "node20",
    outDir: "dist",
    shims: true,
    dts: true,
    sourcemap: true,
    minify: false,
    splitting: false,
    treeshake: false,
    clean: true,
    external: ["../wasm/nize_wasm.js"],
  },
  // CLI build (ESM only, Node-only)
  {
    entry: ["src/cli.ts"],
    format: ["esm"],
    target: "node20",
    outDir: "dist",
    shims: true,
    dts: false,
    sourcemap: true,
    minify: false,
    splitting: false,
    treeshake: true,
    clean: false,
    external: ["commander", "../wasm/nize_wasm.js"],
    outExtension: () => ({
      js: ".cli.js",
    }),
  },
]);
