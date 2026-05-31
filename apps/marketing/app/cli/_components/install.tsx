"use client";

import { useState } from "react";
import { motion } from "framer-motion";
import { Check, Copy } from "lucide-react";

const METHODS = [
  { label: "cargo", cmd: "cargo install vibn", note: "From source (Rust 1.79+)" },
  { label: "brew", cmd: "brew install vibn", note: "macOS · soon" },
  { label: "curl", cmd: "curl -fsSL https://vibn.dev/install.sh | sh", note: "Linux & macOS prebuilt" },
  { label: "scoop", cmd: "scoop install vibn", note: "Windows" },
];

export function Install() {
  const [copied, setCopied] = useState<string | null>(null);
  const copy = async (cmd: string) => {
    await navigator.clipboard.writeText(cmd);
    setCopied(cmd);
    setTimeout(() => setCopied(null), 1500);
  };

  return (
    <section id="install" className="relative py-20">
      <div className="mx-auto max-w-4xl px-6">
        <div className="text-center">
          <div className="inline-flex rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/50 px-3 py-1 text-xs text-[color:var(--color-text-dim)]">
            Install
          </div>
          <h2 className="mt-4 text-3xl font-semibold tracking-tight sm:text-4xl">
            <span className="text-white/85">Pick your</span>{" "}
            <span className="gradient-text">poison.</span>
          </h2>
        </div>

        <div className="mt-10 grid gap-3 sm:grid-cols-2">
          {METHODS.map((m, i) => (
            <motion.div
              key={m.label}
              initial={{ opacity: 0, y: 10 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ duration: 0.3, delay: i * 0.05 }}
              className="rounded-xl border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/40 p-4 backdrop-blur"
            >
              <div className="flex items-center justify-between">
                <span className="rounded-full bg-[color:var(--color-surface-2)] px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider text-[color:var(--color-text-dim)]">
                  {m.label}
                </span>
                <span className="text-[11px] text-[color:var(--color-text-dim)]">{m.note}</span>
              </div>
              <button
                onClick={() => copy(m.cmd)}
                className="mt-3 flex w-full items-center justify-between rounded-lg bg-black/40 px-3 py-2 text-left font-mono text-xs text-white/90 transition hover:bg-black/60"
              >
                <span className="truncate">
                  <span className="text-[color:var(--color-violet)]">$ </span>
                  {m.cmd}
                </span>
                {copied === m.cmd ? (
                  <Check className="h-3.5 w-3.5 shrink-0 text-[color:var(--color-green)]" />
                ) : (
                  <Copy className="h-3.5 w-3.5 shrink-0 text-[color:var(--color-text-dim)]" />
                )}
              </button>
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  );
}
