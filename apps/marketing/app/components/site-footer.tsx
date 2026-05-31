import Link from "next/link";
import { Logo } from "./logo";

export function SiteFooter() {
  return (
    <footer className="relative border-t border-[color:var(--color-border)] bg-[color:var(--color-bg-2)]">
      <div className="mx-auto flex max-w-6xl flex-col items-start justify-between gap-8 px-6 py-12 sm:flex-row sm:items-center">
        <div className="flex items-center gap-3">
          <Logo className="h-6 w-6" />
          <div>
            <div className="text-sm font-semibold">Vibn</div>
            <div className="text-xs text-[color:var(--color-text-dim)]">Local-first AI agent for your desktop.</div>
          </div>
        </div>
        <nav className="flex flex-wrap items-center gap-x-6 gap-y-2 text-sm text-[color:var(--color-text-dim)]">
          <a href="/#features" className="hover:text-white">Features</a>
          <a href="/#models" className="hover:text-white">Models</a>
          <a href="/#tools" className="hover:text-white">Tools & MCP</a>
          <Link href="/cli" className="hover:text-white">CLI</Link>
          <Link href="/changelog" className="hover:text-white">Changelog</Link>
        </nav>
        <div className="text-xs text-[color:var(--color-text-dim)]">© {new Date().getFullYear()} Vibn</div>
      </div>
    </footer>
  );
}
