import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Prisma's generated client imports binary engine files; mark it external
  // so Next/webpack doesn't try to bundle it for serverless functions.
  serverExternalPackages: ["@prisma/client", "@prisma/adapter-neon"]
};

export default nextConfig;
