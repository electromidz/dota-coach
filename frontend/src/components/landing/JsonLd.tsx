import { FAQ_ITEMS } from "@/lib/faq";
import { SITE_DESCRIPTION, SITE_NAME, SITE_URL, absoluteUrl } from "@/lib/site";

/**
 * Structured data for the landing page, emitted as one `@graph` rather than
 * three separate scripts so the nodes can reference each other by `@id` —
 * a crawler that resolves `WebSite.publisher` to the same `Organization`
 * node understands them as one entity instead of three loose claims.
 *
 * What is deliberately **not** here: `AggregateRating` and `Review`. This
 * product has no collected ratings, and marking up ratings that do not exist
 * is a review-spam violation that earns a manual action rather than a rich
 * result. The offer, by contrast, is real and comes from billing
 * configuration.
 *
 * `FAQPage` no longer earns a rich snippet for a site like this one (Google
 * narrowed that to authoritative health and government sources in 2023), but
 * it still describes the page cleanly for AI answer surfaces and costs one
 * script tag.
 */
export function LandingJsonLd({
  priceUsd,
  currency = "USD",
}: {
  priceUsd?: string;
  currency?: string;
}) {
  const organization = {
    "@type": "Organization",
    "@id": `${SITE_URL}/#organization`,
    name: SITE_NAME,
    url: SITE_URL,
    logo: {
      "@type": "ImageObject",
      url: absoluteUrl("/icons/icon.svg"),
    },
    // Valve owns Dota 2; saying so in the graph matches the footer disclaimer
    // and keeps the entity from being conflated with an official product.
    disambiguatingDescription:
      "Independent third-party coaching tool for Dota 2. Not affiliated with Valve Corporation.",
  };

  const website = {
    "@type": "WebSite",
    "@id": `${SITE_URL}/#website`,
    url: SITE_URL,
    name: SITE_NAME,
    description: SITE_DESCRIPTION,
    inLanguage: "en",
    publisher: { "@id": `${SITE_URL}/#organization` },
  };

  const application = {
    "@type": "SoftwareApplication",
    "@id": `${SITE_URL}/#app`,
    name: "Dota Coach — AI Dota 2 Coach",
    url: SITE_URL,
    description: SITE_DESCRIPTION,
    applicationCategory: "GameApplication",
    applicationSubCategory: "Esports coaching and match analysis",
    operatingSystem: "Web browser",
    browserRequirements: "Requires JavaScript and a Steam account",
    publisher: { "@id": `${SITE_URL}/#organization` },
    about: {
      "@type": "VideoGame",
      name: "Dota 2",
      publisher: { "@type": "Organization", name: "Valve Corporation" },
    },
    featureList: [
      "Dota 2 match history analysis",
      "Rank- and role-scoped performance benchmarks",
      "Recurring mistake and pattern detection",
      "Hero pool and patch-aware hero recommendations",
      "A single tracked training focus with progress over time",
    ],
    // Omitted entirely when the backend has not resolved a price: an offer
    // that guesses is worse than no offer node at all.
    ...(priceUsd
      ? {
          offers: {
            "@type": "Offer",
            price: priceUsd,
            priceCurrency: currency,
            category: "subscription",
            availability: "https://schema.org/OnlineOnly",
            url: absoluteUrl("/#pricing"),
          },
        }
      : {}),
  };

  const faq = {
    "@type": "FAQPage",
    "@id": `${SITE_URL}/#faq`,
    mainEntity: FAQ_ITEMS.map((item) => ({
      "@type": "Question",
      name: item.q,
      acceptedAnswer: { "@type": "Answer", text: item.a },
    })),
  };

  const graph = {
    "@context": "https://schema.org",
    "@graph": [organization, website, application, faq],
  };

  return (
    <script
      type="application/ld+json"
      // The payload is built from module constants and backend-supplied
      // numbers, never from user input, and `JSON.stringify` escapes the
      // quotes; `</script>` cannot appear in it.
      dangerouslySetInnerHTML={{ __html: JSON.stringify(graph) }}
    />
  );
}
