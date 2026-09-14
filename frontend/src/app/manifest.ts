import type { MetadataRoute } from "next";

/**
 * The install manifest.
 *
 * A route rather than a static file so the name, colours and start URL cannot
 * drift from `layout.tsx` — both read like configuration and both are wrong the
 * moment they disagree.
 *
 * `display: "standalone"` is what makes the installed app drop the browser
 * chrome, which is the whole point of the mobile-first shell: the bottom tab
 * bar is the navigation once there is no URL bar above it.
 */
export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "AI Dota Coach",
    short_name: "Dota Coach",
    description:
      "A personal AI coach that learns your Dota 2 habits across matches and tells you what to train next.",
    start_url: "/",
    scope: "/",
    display: "standalone",
    orientation: "portrait",
    // Matches --color-base, so the splash screen and the app are the same
    // colour and an install does not flash white on launch.
    background_color: "#1a1b26",
    theme_color: "#1a1b26",
    categories: ["games", "sports", "utilities"],
    icons: [
      { src: "/icons/icon-192.png", sizes: "192x192", type: "image/png" },
      { src: "/icons/icon-512.png", sizes: "512x512", type: "image/png" },
      // Separate artwork rather than the same file marked maskable: a launcher
      // crops this one to a circle, and the mark has to survive that.
      {
        src: "/icons/icon-maskable-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "maskable",
      },
    ],
  };
}
