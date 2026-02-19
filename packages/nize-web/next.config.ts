// @zen-impl: PLAN-012-1.3 — Next.js config with standalone output
// @zen-impl: PLAN-021 — removed basePath (no longer served in iframe)
import type { NextConfig } from "next";

const apiPort = process.env.NIZE_API_PORT || "3001";

const nextConfig: NextConfig = {
  output: "standalone",
  // In dev, proxy /api/* to the local API sidecar so cookies stay
  // first-party (same origin).  All API routes are nested under /api.
  // In Tauri desktop, the API port is discovered via IPC;
  // in cloud, NEXT_PUBLIC_API_URL is used instead.
  async rewrites() {
    return [{ source: "/api/:path*", destination: `http://127.0.0.1:${apiPort}/api/:path*` }];
  },
};

export default nextConfig;
