// @awa-impl: PLAN-012-1.2 — root layout for nize-web
// @awa-impl: CFG-NizeWebAuthContext
// @awa-impl: PLAN-021 — webview bridge injection for Tauri dev builds

import type { Metadata } from "next";
import { Toaster } from "sonner";
import { AuthProvider } from "@/lib/auth-context";
import { WebviewBridgeLoader } from "@/components/WebviewBridgeLoader";
import { DevPanelProvider } from "@/components/dev-panel/dev-panel-provider";
import { DevPanel } from "@/components/dev-panel/dev-panel";
import "./globals.css";

export const metadata: Metadata = {
  title: "Nize - Your AI-Powered Data Hub",
  description: "Manage your data with AI chat",
};

// @awa-impl: DEV-1_AC-3
// @awa-impl: DEV-3_AC-1
// @awa-impl: DEV-3_AC-2
// @awa-impl: DEV-3_AC-3
// @awa-impl: DEV-3_AC-5
// @awa-impl: CHAT-7.1_AC-3
export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="h-full">
      <head>
        {/* Runtime env injected by nize-web-server.mjs before server start */}
        <script src="/__nize-env.js" />
      </head>
      <body className="h-full">
        <AuthProvider>
          <WebviewBridgeLoader />
          <DevPanelProvider>
            <div className="flex flex-col lg:flex-row h-full">
              <div className="flex-1 min-w-0 overflow-auto">{children}</div>
              <DevPanel />
            </div>
            <Toaster position="bottom-right" richColors />
          </DevPanelProvider>
        </AuthProvider>
      </body>
    </html>
  );
}
