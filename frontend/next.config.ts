import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Emits a self-contained server bundle so the production Docker image does
  // not need node_modules.
  output: "standalone",
  reactStrictMode: true,
};

export default nextConfig;
