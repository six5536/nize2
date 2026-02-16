// @zen-impl: PLAN-021 — serve /__nize-env.js in dev (production uses nize-web-server.mjs)
import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

const envPayload = `window.__NIZE_ENV__=${JSON.stringify({ apiPort: process.env.NIZE_API_PORT || "" })};\n`;

export function proxy(request: NextRequest) {
  if (request.nextUrl.pathname === "/__nize-env.js") {
    return new NextResponse(envPayload, {
      headers: {
        "Content-Type": "application/javascript",
        "Cache-Control": "no-cache",
      },
    });
  }
  return NextResponse.next();
}

export const config = {
  matcher: "/__nize-env.js",
};
