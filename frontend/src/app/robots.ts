import type { MetadataRoute } from "next";

import { SITE_URL, absoluteUrl } from "@/lib/site";

/**
 * Only the landing page is public. Every other route in this app is a signed-in
 * screen whose HTML, to a crawler with no session, is an empty shell — indexing
 * a dozen identical skeletons under distinct URLs is how a small site dilutes
 * its own relevance and burns crawl budget on nothing.
 *
 * These paths are already protected server-side; `Disallow` is an indexing
 * instruction, never an access control, and nothing here relies on it for
 * secrecy.
 */
export default function robots(): MetadataRoute.Robots {
  return {
    rules: {
      userAgent: "*",
      allow: "/",
      disallow: [
        "/admin",
        "/billing",
        "/benchmark",
        "/coach",
        "/heroes",
        "/matches",
        "/profile",
        "/offline",
        // The Steam round trip lands back on `/` carrying a one-shot error
        // code. It is the same page; it should not become a second URL.
        "/?error=",
      ],
    },
    sitemap: absoluteUrl("/sitemap.xml"),
    host: SITE_URL,
  };
}
