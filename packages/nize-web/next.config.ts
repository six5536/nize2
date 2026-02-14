// @zen-impl: PLAN-012-1.3 — Next.js config with standalone output
import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  // When running as a Tauri sidecar, nize-web is served under /nize-web/ so
  // that the iframe shares the same origin as the desktop shell.
  ...(process.env.NIZE_WEB_BASE_PATH ? { basePath: process.env.NIZE_WEB_BASE_PATH } : {}),
};

export default nextConfig;
