// @zen-impl: PLAN-012-2.2 — build script for nize-web sidecar bundle
//
// Builds the Next.js standalone output and copies it (along with the wrapper
// script) into the Tauri resources directory so it can be bundled with the app.
import { execSync } from "node:child_process";
import { cpSync, mkdirSync, rmSync, existsSync, copyFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const nizeWebDir = join(__dirname, "..", "..", "nize-web");
const outDir = join(__dirname, "..", "..", "..", "crates", "app", "nize_desktop", "resources", "nize-web");

// Clean output directory
if (existsSync(outDir)) {
  rmSync(outDir, { recursive: true });
}
mkdirSync(outDir, { recursive: true });

// Build Next.js
console.log("Building nize-web…");
execSync("npm run build", { cwd: nizeWebDir, stdio: "inherit" });

// Copy standalone output
const standaloneDir = join(nizeWebDir, ".next", "standalone");
cpSync(standaloneDir, join(outDir, "standalone"), { recursive: true });

// Copy static assets into the monorepo-nested standalone directory.
// Next.js standalone in a workspace nests the app at packages/nize-web/.
const nestedAppDir = join(outDir, "standalone", "packages", "nize-web");
const staticSrc = join(nizeWebDir, ".next", "static");
if (existsSync(staticSrc)) {
  cpSync(staticSrc, join(nestedAppDir, ".next", "static"), {
    recursive: true,
  });
}

// Copy public directory into standalone if it exists
const publicSrc = join(nizeWebDir, "public");
if (existsSync(publicSrc)) {
  cpSync(publicSrc, join(nestedAppDir, "public"), { recursive: true });
}

// Copy wrapper script
copyFileSync(join(nizeWebDir, "scripts", "nize-web-server.mjs"), join(outDir, "nize-web-server.mjs"));

console.log(`nize-web built and copied to ${outDir}`);
