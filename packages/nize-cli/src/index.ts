import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const wasm = require("../wasm/nize_wasm.js") as typeof import("../wasm/nize_wasm.js");

/**
 * Get the version of the @six5536/nize library.
 *
 * @returns Version string (e.g. "0.1.0").
 */
export function version(): string {
  return wasm.version();
}
