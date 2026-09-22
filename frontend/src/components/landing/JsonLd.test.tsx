import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FAQ_ITEMS } from "@/lib/faq";

import { LandingJsonLd } from "./JsonLd";

/** Parses the single `application/ld+json` block the component renders. */
function graphOf(container: HTMLElement): Record<string, unknown>[] {
  const script = container.querySelector(
    'script[type="application/ld+json"]',
  );
  expect(script).not.toBeNull();

  const parsed = JSON.parse(script!.textContent ?? "") as {
    "@graph": Record<string, unknown>[];
  };
  return parsed["@graph"];
}

function node(container: HTMLElement, type: string) {
  return graphOf(container).find((entry) => entry["@type"] === type);
}

describe("LandingJsonLd", () => {
  it("emits one parseable graph with the entity types crawlers look for", () => {
    const { container } = render(<LandingJsonLd />);

    const types = graphOf(container).map((entry) => entry["@type"]);
    expect(types).toEqual([
      "Organization",
      "WebSite",
      "SoftwareApplication",
      "FAQPage",
    ]);
  });

  it("marks up exactly the questions the page displays", () => {
    const { container } = render(<LandingJsonLd />);

    const faq = node(container, "FAQPage") as {
      mainEntity: { name: string; acceptedAnswer: { text: string } }[];
    };

    expect(faq.mainEntity).toHaveLength(FAQ_ITEMS.length);
    expect(faq.mainEntity.map((q) => q.name)).toEqual(
      FAQ_ITEMS.map((item) => item.q),
    );
    expect(faq.mainEntity[0].acceptedAnswer.text).toBe(FAQ_ITEMS[0].a);
  });

  it("omits the offer entirely when the backend price is unavailable", () => {
    const { container } = render(<LandingJsonLd />);

    expect(node(container, "SoftwareApplication")).not.toHaveProperty("offers");
  });

  it("uses the backend price rather than a hard-coded one", () => {
    const { container } = render(
      <LandingJsonLd priceUsd="9.98" currency="USD" />,
    );

    expect(node(container, "SoftwareApplication")).toMatchObject({
      offers: { price: "9.98", priceCurrency: "USD" },
    });
  });

  /**
   * Fabricated ratings are a review-spam violation, and this product has
   * collected none. The guard is a test rather than a comment because the
   * temptation to add stars to a landing page outlives any comment.
   */
  it("claims no ratings or reviews", () => {
    const { container } = render(<LandingJsonLd priceUsd="9.98" />);
    const serialized = JSON.stringify(graphOf(container));

    expect(serialized).not.toMatch(/aggregateRating|"Review"|ratingValue/i);
  });
});
