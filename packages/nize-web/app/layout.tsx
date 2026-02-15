// @zen-impl: PLAN-012-1.2 — root layout for nize-web
// @zen-impl: CFG-NizeWebAuthContext

import { AuthProvider } from "@/lib/auth-context";
import "./globals.css";

export const metadata = {
  title: "nize-web",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className="h-full">
      <head>
        {/* Runtime env injected by nize-web-server.mjs before server start */}
        <script src="/__nize-env.js" />
      </head>
      <body className="h-full">
        <AuthProvider>
          <div className="flex flex-col h-full">
            <div className="flex-1 min-w-0 overflow-auto">{children}</div>
          </div>
        </AuthProvider>
      </body>
    </html>
  );
}
