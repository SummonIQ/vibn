import type { Metadata } from "next";
import { GeistSans } from "geist/font/sans";
import { GeistMono } from "geist/font/mono";
import "./globals.css";
import { SiteHeader } from "./components/site-header";
import { SiteFooter } from "./components/site-footer";
import { AppAnalyticsProvider } from "./components/analytics-provider";

const description =
  "Vibn is a local-first AI agent for your desktop. Runs on your own Ollama models with full tool access, MCP support, and zero cloud round-trips — so it can touch the work you can't send to a SaaS: code, contracts, patient notes, financials, anything.";

export const metadata: Metadata = {
  metadataBase: new URL("https://vibn.dev"),
  title: {
    default: "Vibn — Local-first AI agent for your desktop",
    template: "%s · Vibn",
  },
  description,
  keywords: [
    "local AI agent",
    "private AI assistant",
    "Ollama desktop app",
    "HIPAA-friendly AI",
    "attorney-client privileged AI",
    "offline AI",
    "MCP",
    "agentic IDE",
    "AI coding agent",
    "vibn",
  ],
  openGraph: {
    type: "website",
    locale: "en_US",
    url: "https://vibn.dev",
    siteName: "Vibn",
    title: "Vibn — Local-first AI agent for your desktop",
    description,
    images: [{ url: "/og-image.png", width: 1200, height: 630, alt: "Vibn — local-first AI agent" }],
  },
  twitter: {
    card: "summary_large_image",
    title: "Vibn — Local-first AI agent for your desktop",
    description,
    images: ["/og-image.png"],
  },
  robots: { index: true, follow: true },
  manifest: "/site.webmanifest",
  icons: {
    icon: [
      { url: "/favicon.svg", type: "image/svg+xml" },
      { url: "/favicon-32x32.png", sizes: "32x32", type: "image/png" },
      { url: "/favicon-16x16.png", sizes: "16x16", type: "image/png" },
    ],
    apple: "/apple-touch-icon.png",
    shortcut: "/favicon.ico",
  },
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className={`${GeistSans.variable} ${GeistMono.variable}`}>
      <body className="min-h-screen antialiased">
        <AppAnalyticsProvider>
          <SiteHeader />
          <main className="relative">{children}</main>
          <SiteFooter />
        </AppAnalyticsProvider>
      </body>
    </html>
  );
}
