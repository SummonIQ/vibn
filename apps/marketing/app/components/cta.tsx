import { Apple, Download, Terminal } from "lucide-react";
import Link from "next/link";
import { Button } from "./ui/button";

export function CTA() {
  return (
    <section id="download" className="relative py-28">
      <div className="mx-auto max-w-4xl px-6">
        <div className="relative overflow-hidden rounded-3xl border border-[color:var(--color-border)] bg-gradient-to-br from-[color:var(--color-surface)] to-[color:var(--color-bg)] p-10 sm:p-14 ring-glow">
          <div
            aria-hidden
            className="absolute -right-40 -top-40 h-[420px] w-[420px] rounded-full blur-3xl"
            style={{ background: "radial-gradient(circle, rgba(167,139,250,0.4), transparent 60%)" }}
          />
          <div
            aria-hidden
            className="absolute -bottom-40 -left-40 h-[420px] w-[420px] rounded-full blur-3xl"
            style={{ background: "radial-gradient(circle, rgba(34,211,238,0.3), transparent 60%)" }}
          />

          <div className="relative">
            <h2 className="text-balance text-4xl font-semibold tracking-tight sm:text-5xl">
              <span className="text-white/85">Your work.</span>{" "}
              <span className="gradient-text">Your hardware. Your data.</span>
            </h2>
            <p className="mt-4 max-w-xl text-pretty text-[color:var(--color-text-dim)]">
              One download. Your machine runs the model, holds the files, and keeps the transcripts. Free, open source, and built to stay that way &mdash; whether the work is code, contracts, or charts.
            </p>

            <div className="mt-8 overflow-hidden rounded-2xl border border-[color:var(--color-border)] bg-black/40 p-4 sm:p-5">
              <div className="flex items-center gap-2 text-[11px] uppercase tracking-wider text-[color:var(--color-text-dim)]">
                <Apple className="h-3.5 w-3.5" />
                Install on macOS (Apple Silicon)
              </div>
              <div className="mt-2 font-mono text-sm text-white sm:text-base">
                <span className="text-[color:var(--color-violet)]">$</span> curl -fsSL https://vibn.dev/install.sh | sh
              </div>
              <p className="mt-2 text-xs text-[color:var(--color-text-dim)]">
                One paste. Downloads the app, drops it in <code className="font-mono">/Applications</code>, opens it.
              </p>
            </div>

            <div className="mt-5 flex flex-wrap items-center gap-3">
              <Button asChild variant="secondary" size="lg">
                <a href="https://50cbcsvzhpu0fjiw.public.blob.vercel-storage.com/v0.1.0/Vibn_0.1.0_aarch64.dmg">
                  <Download className="h-4 w-4" />
                  Direct .dmg download
                </a>
              </Button>
              <Button asChild variant="ghost" size="lg" className="px-3">
                <Link href="/cli">
                  <Terminal className="h-4 w-4" />
                  Just give me the CLI
                </Link>
              </Button>
            </div>

            <p className="mt-3 text-xs text-[color:var(--color-text-dim)]">
              If you grab the <code className="font-mono">.dmg</code> through the browser, macOS marks it as quarantined and refuses to open it. The install script avoids that. Linux & Windows builds coming soon.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
