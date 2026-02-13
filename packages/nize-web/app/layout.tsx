// @zen-impl: PLAN-012-1.2 — root layout for nize-web

export const metadata = {
  title: "nize-web",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
