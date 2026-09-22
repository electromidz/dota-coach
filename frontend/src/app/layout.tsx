import type { Metadata, Viewport } from "next";
import { Chakra_Petch, Russo_One } from "next/font/google";

import { ServiceWorker } from "@/components/shell/ServiceWorker";
import {
  SITE_DESCRIPTION,
  SITE_NAME,
  SITE_TITLE,
  SITE_URL,
} from "@/lib/site";

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
  // Every `og:`/`twitter:` image and every canonical below resolves against
  // this. Without it Next emits relative URLs, which crawlers and social
  // scrapers both refuse to follow.
  metadataBase: new URL(SITE_URL),
  title: {
    default: SITE_TITLE,
    template: "%s · Dota Coach",
  },
  description: SITE_DESCRIPTION,
  applicationName: SITE_NAME,
  // Signed-in screens carry no keywords worth competing on, so the canonical
  // set here is the landing page's and every app route inherits a self-
  // referencing one from `generateMetadata` where it matters.
  alternates: { canonical: "/" },
  authors: [{ name: SITE_NAME, url: SITE_URL }],
  creator: SITE_NAME,
  publisher: SITE_NAME,
  category: "Gaming",
  openGraph: {
    type: "website",
    siteName: SITE_NAME,
    locale: "en_US",
    url: SITE_URL,
    title: SITE_TITLE,
    description: SITE_DESCRIPTION,
  },
  twitter: {
    card: "summary_large_image",
    title: SITE_TITLE,
    description: SITE_DESCRIPTION,
  },
  robots: {
    index: true,
    follow: true,
    googleBot: {
      index: true,
      follow: true,
      // Lets Google show a full-length description and a large preview image
      // instead of the truncated defaults it falls back to.
      "max-snippet": -1,
      "max-image-preview": "large",
      "max-video-preview": -1,
    },
  },
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
