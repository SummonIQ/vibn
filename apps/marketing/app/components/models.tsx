"use client";

import { motion } from "framer-motion";

const MODELS = [
  { name: "qwen2.5-coder:7b", role: "Default coder", size: "4.7 GB", strength: "fast" },
  { name: "qwen2.5-coder:14b", role: "Bigger reasoner", size: "9 GB", strength: "balanced" },
  { name: "deepseek-coder-v2:16b", role: "Heavy lifting", size: "9 GB", strength: "smart" },
  { name: "llama3.2:3b", role: "Ultra-fast", size: "2 GB", strength: "tiny" },
  { name: "gpt-oss:20b", role: "Open source GPT", size: "12 GB", strength: "premium" },
  { name: "mistral-small:24b", role: "Long context", size: "14 GB", strength: "context" },
];

const STRENGTH_COLORS: Record<string, string> = {
  fast: "var(--color-green)",
  balanced: "var(--color-cyan)",
  smart: "var(--color-violet)",
  tiny: "var(--color-amber)",
  premium: "var(--color-pink)",
  context: "var(--color-cyan)",
};

export function Models() {
  return (
    <section id="models" className="relative py-28">
      <div className="absolute inset-0 -z-10 dot-grid opacity-30" />
      <div className="mx-auto max-w-6xl px-6">
        <div className="mx-auto max-w-2xl text-center">
          <div className="inline-flex rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/50 px-3 py-1 text-xs text-[color:var(--color-text-dim)] backdrop-blur">
            Models
          </div>
          <h2 className="mt-4 text-balance text-4xl font-semibold tracking-tight sm:text-5xl">
            <span className="text-white/85">Pick a model.</span>{" "}
            <span className="gradient-text">Switch any time.</span>
          </h2>
          <p className="mt-4 text-pretty text-[color:var(--color-text-dim)]">
            Pick a coder model for code, a generalist for prose, a tiny one for fast triage. Any Ollama model works out of the box, and your hardware decides the ceiling.
          </p>
        </div>

        <div className="mt-14 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {MODELS.map((m, i) => (
            <motion.div
              key={m.name}
              initial={{ opacity: 0, y: 14 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true, margin: "-60px" }}
              transition={{ duration: 0.7, delay: i * 0.06, ease: [0.22, 1, 0.36, 1] }}
              className="group flex items-center justify-between rounded-xl border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/40 px-4 py-3 backdrop-blur transition hover:border-[color:var(--color-violet)]/40"
            >
              <div className="min-w-0">
                <div className="font-mono text-sm text-white">{m.name}</div>
                <div className="mt-0.5 truncate text-xs text-[color:var(--color-text-dim)]">{m.role} · {m.size}</div>
              </div>
              <span
                className="ml-3 shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide"
                style={{
                  color: STRENGTH_COLORS[m.strength],
                  background: `${STRENGTH_COLORS[m.strength]}1a`,
                }}
              >
                {m.strength}
              </span>
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  );
}
