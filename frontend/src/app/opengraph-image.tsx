import { ImageResponse } from "next/og";

/**
 * The social card, generated rather than stored as a binary.
 *
 * A link with no `og:image` renders as a bare text row on every platform that
 * matters, which is most of the referral traffic a launch gets. Generating it
 * keeps the card in sync with the copy — there is no stale PNG to forget to
 * re-export — and adds no asset to the repo.
 *
 * X/Twitter falls back to `og:image` when `twitter:image` is absent, so this
 * one file covers both.
 *
 * Satori (what `ImageResponse` renders with) supports a subset of CSS and
 * requires an explicit `display` on every element with more than one child —
 * hence the flex containers that would otherwise be redundant.
 */
export const alt =
  "Dota Coach — the AI Dota 2 coach that finds the mistake you keep making";

export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default function OpengraphImage() {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
          padding: "80px",
          // --color-base, with the same two ambient glows the landing backdrop
          // uses, so the card looks like the page it opens.
          background: "#1a1b26",
          backgroundImage:
            "radial-gradient(70% 90% at 15% 0%, rgba(187,154,247,0.28), transparent 60%)," +
            "radial-gradient(60% 80% at 100% 100%, rgba(122,162,247,0.22), transparent 60%)",
          color: "#c0caf5",
          fontFamily: "sans-serif",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "16px",
            fontSize: 30,
            letterSpacing: "0.04em",
            color: "#7aa2f7",
          }}
        >
          <div
            style={{
              display: "flex",
              width: "20px",
              height: "20px",
              borderRadius: "999px",
              background: "#bb9af7",
            }}
          />
          DOTA COACH
        </div>

        <div
          style={{
            display: "flex",
            marginTop: "36px",
            fontSize: 74,
            lineHeight: 1.1,
            fontWeight: 700,
            color: "#c0caf5",
            maxWidth: "900px",
          }}
        >
          The AI Dota 2 coach that finds the mistake you keep making
        </div>

        <div
          style={{
            display: "flex",
            marginTop: "32px",
            fontSize: 30,
            lineHeight: 1.4,
            color: "#9aa5ce",
            maxWidth: "860px",
          }}
        >
          Benchmarks from your own rank. One thing to train next. Progress you
          can actually see.
        </div>
      </div>
    ),
    size,
  );
}
