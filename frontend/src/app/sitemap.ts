import type { MetadataRoute } from "next";

import { SITE_URL } from "@/lib/site";

/**
 * One entry, because there is one indexable page.
 *
 * A sitemap padded with the app's signed-in routes would contradict
 * `robots.ts` and tell Search Console to crawl URLs it is simultaneously
 * told to skip. When marketing pages (pricing, guides, a blog) get their own
 * routes, they belong here — the in-page anchors on the landing page do not.
 */
export default function sitemap(): MetadataRoute.Sitemap {
  return [
    {
      url: SITE_URL,
      lastModified: new Date(),
      changeFrequency: "weekly",
      priority: 1,
    },
  ];
}
