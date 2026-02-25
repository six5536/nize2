// @awa-impl: PLAN-012-1.2 — root page redirects to /chat

import { redirect } from "next/navigation";

export default function Home() {
  redirect("/chat");
}
