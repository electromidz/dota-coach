import type { Metadata, Viewport } from "next";
import { Chakra_Petch, Russo_One } from "next/font/google";

import { ServiceWorker } from "@/components/shell/ServiceWorker";

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
  manifest: "/manifest.webmanifest",
  icons: {
    icon: [
      { url: "/icons/favicon-32.png", sizes: "32x32", type: "image/png" },
      // Offered alongside the PNG so a browser that prefers vector takes it.
      { url: "/icons/icon.svg", type: "image/svg+xml" },
    ],
    apple: "/icons/apple-touch-icon.png",
  },
  appleWebApp: {
    // iOS has no manifest: installability, the status bar and the launch title
    // come from these instead.
    capable: true,
    title: "Dota Coach",
    statusBarStyle: "black-translucent",
  },
  // Phone numbers are not a thing in this app; Safari's auto-detection only
  // ever turns a match id into a broken tel: link.
  formatDetection: { telephone: false },
};

export const viewport: Viewport = {
  // Matches --color-base, so the browser chrome blends into the app shell.
  themeColor: "#1a1b26",
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
      <body className="min-h-dvh font-sans antialiased">
        {children}
        <ServiceWorker />
      </body>
    </html>
  );
}
