"use client";

import { motion } from "framer-motion";

const COMMANDS = [
  { cmd: "/model", desc: "Open the model picker — switch any local Ollama model live" },
  { cmd: "/marketplace", desc: "Browse MCP servers and one-click install" },
  { cmd: "/skills", desc: "Toggle skills (workflows, plans, templates) on/off" },
  { cmd: "/compact", desc: "Manually trigger context compaction" },
  { cmd: "/observations", desc: "Open project + global observation files" },
  { cmd: "/generate-training-data", desc: "Generate fine-tuning seeds from your transcripts" },
  { cmd: "/clear", desc: "Reset the session, keep observations" },
  { cmd: "/cost", desc: "Show token budget and usage for the current session" },
];

export function SlashCommands() {
  return (
    <section className="relative py-20">
      <div className="mx-auto max-w-4xl px-6">
        <div className="text-center">
          <div className="inline-flex rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/50 px-3 py-1 text-xs text-[color:var(--color-text-dim)]">
            Slash commands
          </div>
          <h2 className="mt-4 text-3xl font-semibold tracking-tight sm:text-4xl">
            <span className="gradient-text">Built-in commands.</span>{" "}
            <span className="text-white/85">All keyboard-first.</span>
          </h2>
        </div>

        <div className="mt-10 overflow-hidden rounded-2xl border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/40 backdrop-blur">
          {COMMANDS.map((c, i) => (
            <motion.div
              key={c.cmd}
              initial={{ opacity: 0, x: -8 }}
              whileInView={{ opacity: 1, x: 0 }}
              viewport={{ once: true, margin: "-60px" }}
              transition={{ duration: 0.25, delay: i * 0.03 }}
              className="flex items-center gap-4 border-b border-[color:var(--color-border)]/60 px-5 py-3 last:border-b-0 hover:bg-white/[0.02]"
            >
              <span className="w-44 shrink-0 font-mono text-sm text-[color:var(--color-cyan)]">{c.cmd}</span>
              <span className="text-sm text-[color:var(--color-text-dim)]">{c.desc}</span>
            </motion.div>
          ))}
        </div>

        <p className="mt-6 text-center text-sm text-[color:var(--color-text-dim)]">
          Plus everything you wire up in <span className="font-mono text-white">~/.vibn/config.json</span>.
        </p>
      </div>
    </section>
  );
}
