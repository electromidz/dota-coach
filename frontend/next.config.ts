import type { NextConfig } from "next";

/**
 * Response headers for every page.
 *
 * The app renders no third-party content, loads no external script and embeds
 * no remote asset — fonts are self-hosted by `next/font` at build time — so the
 * policy can be strict without a list of exceptions. The one dynamic origin is
 * the backend, which the browser reaches with `fetch`, hence `connect-src`.
 *
 * `'unsafe-inline'` on styles is Next's inline critical CSS, and
 * `'unsafe-eval'` is deliberately absent: nothing here evaluates strings.
 */
function contentSecurityPolicy(): string {
  // Same value the client bundle is built against, so the policy cannot
  // disagree with what the app actually calls.
  const api = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

  // The dev server compiles and hot-reloads in the browser: React Refresh
  // evaluates strings, and HMR talks over a websocket. Both are development
  // machinery, and neither is allowed in the build that ships.
  const dev = process.env.NODE_ENV !== "production";
  const script = dev
    ? "script-src 'self' 'unsafe-inline' 'unsafe-eval'"
    : "script-src 'self' 'unsafe-inline'";
  const sockets = dev ? " ws: wss:" : "";

  return [
    "default-src 'self'",
    "base-uri 'self'",
    // Next hydration ships an inline bootstrap script; scripts otherwise come
    // from this origin only.
    script,
    "style-src 'self' 'unsafe-inline'",
    // Hero portraits and Steam avatars, which Valve serves from several
    // interchangeable CDN hostnames.
    "img-src 'self' data: https://*.steamstatic.com https://steamcdn-a.akamaihd.net",
    "font-src 'self'",
    `connect-src 'self' ${api}${sockets}`,
    "manifest-src 'self'",
    // Nothing is embedded, and nothing may embed this app.
    "frame-ancestors 'none'",
    "object-src 'none'",
    "form-action 'self'",
  ].join("; ");
}

const nextConfig: NextConfig = {
  // Emits a self-contained server bundle so the production Docker image does
  // not need node_modules.
  output: "standalone",
  reactStrictMode: true,
  // The server's identity is not a secret worth leaking to every scanner.
  poweredByHeader: false,

  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          { key: "Content-Security-Policy", value: contentSecurityPolicy() },
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
          { key: "X-Frame-Options", value: "DENY" },
          {
            key: "Permissions-Policy",
            value: "camera=(), microphone=(), geolocation=(), payment=()",
          },
        ],
      },
      {
        // The worker controls every navigation in its scope, so it must never
        // be served from a stale cache after a deploy.
        source: "/sw.js",
        headers: [
          { key: "Cache-Control", value: "no-cache, no-store, must-revalidate" },
        ],
      },
    ];
  },
};

export default nextConfig;
