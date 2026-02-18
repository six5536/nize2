// @zen-component: PLAN-027-NextAdapter

import { chatApp } from "@six5536/nize-chat";

// Next.js Route Handlers use Web Standard Request/Response — same as Hono.
// Route Handlers take precedence over next.config.ts rewrites, so /api/chat
// is intercepted here while all other /api/* paths continue proxying to the
// Rust API.
export const POST = (request: Request) => chatApp.fetch(request);
