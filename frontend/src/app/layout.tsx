import type { Metadata, Viewport } from "next";
import { Chakra_Petch, Russo_One } from "next/font/google";

import "./globals.css";

/* Self-hosted by next/font at build time: no runtime request to Google, no
   layout shift, and nothing for a CSP to allow. */
// Only the three weights the UI actually uses (400 body, 500 medium,
// 600 semibold). Shipping the full family cost ~40 unused font files.
const chakra = Chakra_Petch({
  subsets: ["latin"],
  weight: ["400", "500", "600"],
  variable: "--font-chakra",
  display: "swap",
});

const russo = Russo_One({
  subsets: ["latin"],
  weight: "400",
  variable: "--font-russo",
  display: "swap",
});

export const metadata: Metadata = {
  title: {
    default: "AI Dota Coach",
    template: "%s · AI Dota Coach",
  },
  description:
    "A personal AI coach that learns your Dota 2 habits across matches and tells you what to train next.",
  applicationName: "AI Dota Coach",
};

export const viewport: Viewport = {
  themeColor: "#08080f",
  width: "device-width",
  initialScale: 1,
  // Lets the layout paint under notches when installed as a PWA.
  viewportFit: "cover",
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className={`${chakra.variable} ${russo.variable}`}>
      <body className="min-h-dvh font-sans antialiased">{children}</body>
    </html>
  );
}
