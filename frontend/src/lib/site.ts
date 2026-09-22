/**
 * Canonical identity of the deployed site.
 *
 * Search engines, `metadataBase`, the sitemap, `robots.txt` and the JSON-LD
 * graph all have to agree on one absolute origin — a canonical that disagrees
 * with the host actually serving the page is the fastest way to have a
 * landing page de-duplicated out of the index. So the origin is read from one
 * place, at build time, rather than hard-coded per file.
 *
 * `NEXT_PUBLIC_` because `sitemap.ts` and `robots.ts` are evaluated on the
 * server while `JsonLd` renders inside the page tree; both need the value and
 * it is public information either way.
 */
export const SITE_URL = (
  process.env.NEXT_PUBLIC_SITE_URL ?? "https://dota-coach-ivory.vercel.app"
).replace(/\/+$/, "");

export const SITE_NAME = "Dota Coach";

/** Used verbatim as the `<title>` default and as the JSON-LD app name. */
export const SITE_TITLE =
  "AI Dota Coach — Personal Dota 2 Coaching From Your Match History";

/** Kept under ~155 characters so Google shows it whole rather than truncating. */
export const SITE_DESCRIPTION =
  "An AI Dota 2 coach that reads your match history, benchmarks you against your own rank, finds the mistake you keep repeating, and gives you one thing to train.";

/** Absolute URL for a path on this site. */
export function absoluteUrl(path: string): string {
  return `${SITE_URL}${path.startsWith("/") ? path : `/${path}`}`;
}
