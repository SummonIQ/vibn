import { betterAuth } from "better-auth";
import { prismaAdapter } from "better-auth/adapters/prisma";
import { prisma } from "@/lib/prisma";

export const auth = betterAuth({
  secret: process.env.BETTER_AUTH_SECRET,
  baseURL: process.env.BETTER_AUTH_URL,

  database: prismaAdapter(prisma, {
    provider: "postgresql"
  }),

  emailAndPassword: {
    enabled: true
  },

  // Trusted origins for the vibn Tauri desktop client. Tauri webviews load
  // from custom schemes / localhost ports depending on platform & dev mode:
  //   - vibn://                  custom scheme (production)
  //   - tauri://localhost        macOS/Linux production webview origin
  //   - http://tauri.localhost   Windows production webview origin
  //   - http://localhost:1420    Tauri dev server default
  trustedOrigins: [
    "vibn://",
    "tauri://localhost",
    "http://tauri.localhost",
    "http://localhost:1420"
  ]
});
