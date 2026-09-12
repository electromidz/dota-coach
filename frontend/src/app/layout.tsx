import type { Metadata, Viewport } from "next";

import "./globals.css";

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
  themeColor: "#0a0b0f",
  width: "device-width",
  initialScale: 1,
  // Lets the layout paint under notches when installed as a PWA.
  viewportFit: "cover",
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body className="min-h-dvh font-sans antialiased">{children}</body>
    </html>
  );
}
