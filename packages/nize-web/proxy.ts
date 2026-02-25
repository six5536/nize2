// @awa-impl: PLAN-021 — serve /__nize-env.js in dev (production uses nize-web-server.mjs)
// @awa-component: AUTH-Middleware
import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

const envPayload = `window.__NIZE_ENV__=${JSON.stringify({ apiPort: process.env.NIZE_API_PORT || "" })};\n`;

// httpOnly cookie name set by API on login/register
const ACCESS_TOKEN_COOKIE = "nize_access";

// Routes that require authentication (redirect to login if not authenticated)
const PROTECTED_ROUTES = ["/chat", "/settings"];

// Admin routes return 404 when not authenticated (hide existence of admin URLs)
const ADMIN_ROUTES = ["/admin"];

// Routes that should redirect to chat if already authenticated
const AUTH_ROUTES = ["/login", "/register"];

// @awa-impl: AUTH-2_AC-1
// @awa-impl: PRM-7_AC-6
export function proxy(request: NextRequest) {
  // Serve /__nize-env.js for Tauri dev builds
  if (request.nextUrl.pathname === "/__nize-env.js") {
    return new NextResponse(envPayload, {
      headers: {
        "Content-Type": "application/javascript",
        "Cache-Control": "no-cache",
      },
    });
  }

  const { pathname } = request.nextUrl;

  // Check for httpOnly access token cookie (set by API)
  const hasSession = !!request.cookies.get(ACCESS_TOKEN_COOKIE)?.value;

  // Root path redirects to chat (or login if not authenticated)
  if (pathname === "/") {
    const destination = hasSession ? "/chat" : "/login";
    return NextResponse.redirect(new URL(destination, request.url));
  }

  // Admin routes: return 404 if not authenticated (hide that admin routes exist)
  if (ADMIN_ROUTES.some((route) => pathname.startsWith(route))) {
    if (!hasSession) {
      // Return 404 to hide admin route existence
      return NextResponse.rewrite(new URL("/not-found", request.url));
    }
  }

  // Protected routes: redirect to login if not authenticated
  if (PROTECTED_ROUTES.some((route) => pathname.startsWith(route))) {
    if (!hasSession) {
      const loginUrl = new URL("/login", request.url);
      loginUrl.searchParams.set("callbackUrl", pathname);
      return NextResponse.redirect(loginUrl);
    }
  }

  // Auth routes: redirect to chat if already authenticated
  if (AUTH_ROUTES.some((route) => pathname === route)) {
    if (hasSession) {
      return NextResponse.redirect(new URL("/chat", request.url));
    }
  }

  return NextResponse.next();
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico|api).*)"],
};
