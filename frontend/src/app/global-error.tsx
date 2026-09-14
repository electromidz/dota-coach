"use client";

/**
 * The last boundary: the root layout itself failed, so there is no shell, no
 * fonts and no `globals.css` to rely on.
 *
 * Everything here is inline and self-contained for that reason. It is not
 * meant to be pretty — it is meant to render when nothing else can.
 */
export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <html lang="en">
      <body
        style={{
          minHeight: "100dvh",
          margin: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          padding: "2rem",
          background: "#1a1b26",
          color: "#c0caf5",
          fontFamily: "system-ui, sans-serif",
        }}
      >
        <main style={{ maxWidth: "32rem" }}>
          <h1 style={{ fontSize: "1.5rem", marginBottom: "0.75rem" }}>
            AI Dota Coach could not start
          </h1>
          <p style={{ lineHeight: 1.6, marginBottom: "1.5rem" }}>
            Something failed before the app could render. Reloading usually
            fixes it.
          </p>

          <button
            onClick={reset}
            style={{
              minHeight: "2.75rem",
              padding: "0 1.5rem",
              borderRadius: "0.75rem",
              border: "none",
              cursor: "pointer",
              background: "#bb9af7",
              color: "#1a1b26",
              fontWeight: 600,
            }}
          >
            Reload
          </button>

          {error.digest && (
            <p
              style={{
                marginTop: "1.5rem",
                fontFamily: "ui-monospace, monospace",
                fontSize: "0.75rem",
                opacity: 0.7,
              }}
            >
              Reference: {error.digest}
            </p>
          )}
        </main>
      </body>
    </html>
  );
}
