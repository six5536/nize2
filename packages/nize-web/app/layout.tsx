// @zen-impl: PLAN-012-1.2 — root layout for nize-web
// @zen-impl: CFG-NizeWebAuthContext

import { AuthProvider } from "@/lib/auth-context";

export const metadata = {
  title: "nize-web",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        {/* Runtime env injected by nize-web-server.mjs before server start */}
        <script src="/__nize-env.js" />
      </head>
      <body>
        <AuthProvider>{children}</AuthProvider>
      </body>
    </html>
  );
}
