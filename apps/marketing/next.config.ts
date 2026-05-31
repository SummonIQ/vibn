import type { NextConfig } from "next";

const config: NextConfig = {
  devIndicators: false,
  reactStrictMode: true,
  typescript: { ignoreBuildErrors: true },
};

export default config;
