"use client";

import Link from "next/link";
import { Logo, WordMark } from "./logo";
import { Button } from "./ui/button";

export function SiteHeader() {
  return (
    <header className="fixed inset-x-0 top-0 z-50">
      <div className="mx-auto mt-3 flex max-w-6xl items-center justify-between rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-bg)]/65 px-4 py-2 backdrop-blur-xl">
        <Link href="/" className="flex items-center" aria-label="vibn home">
          <Logo className="h-7 w-7 sm:hidden" />
          <WordMark size={28} className="hidden text-white sm:inline-flex" />
        </Link>
        <nav className="hidden items-center gap-1 text-sm text-[color:var(--color-text-dim)] sm:flex">
          <a href="/#features" className="rounded-full px-3 py-1.5 transition hover:bg-white/5 hover:text-white">Features</a>
          <a href="/#models" className="rounded-full px-3 py-1.5 transition hover:bg-white/5 hover:text-white">Models</a>
          <a href="/#tools" className="rounded-full px-3 py-1.5 transition hover:bg-white/5 hover:text-white">Tools & MCP</a>
          <Link href="/cli" className="rounded-full px-3 py-1.5 transition hover:bg-white/5 hover:text-white">CLI</Link>
          <Link href="/changelog" className="rounded-full px-3 py-1.5 transition hover:bg-white/5 hover:text-white">Changelog</Link>
        </nav>
        <div className="flex items-center gap-2">
          <Button asChild size="sm">
            <a href="#download">Download</a>
          </Button>
        </div>
      </div>
    </header>
  );
}
