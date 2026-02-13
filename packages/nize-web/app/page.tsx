// @zen-impl: PLAN-012-1.2 — hello world page

export default function Home() {
  return (
    <main
      style={{
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        minHeight: "100vh",
        fontFamily: "system-ui, sans-serif",
      }}
    >
      <h1>Hello from nize-web</h1>
    </main>
  );
}
