"use client";

import { motion } from "framer-motion";
import { Boxes, Command, Database, KeyRound, Layers3, Zap } from "lucide-react";

const ITEMS = [
  { icon: Command, color: "var(--color-cyan)", title: "Slash commands", body: "Fuzzy-completed command palette. /model, /skills, /marketplace, /generate-training-data, anything you wire up." },
  { icon: Layers3, color: "var(--color-violet)", title: "Ratatui fullscreen TUI", body: "Smooth 60fps in your terminal. Background tasks, training-data review, MCP browser — all keyboard-driven." },
  { icon: Zap, color: "var(--color-green)", title: "Smart compaction", body: "Auto-compacts at 90% of context. Collapses tool call/result pairs into one-liner summaries so long sessions keep going." },
  { icon: Database, color: "var(--color-pink)", title: "Transcripts & observations", body: "JSONL transcripts in ~/.vibn/transcripts/. Per-project OBSERVATIONS.md that the agent updates as it learns." },
  { icon: KeyRound, color: "var(--color-amber)", title: "Permissions per tool", body: "Allow, deny, or ask — per tool, per directory, per session. Hooks fire on every event in the lifecycle." },
  { icon: Boxes, color: "var(--color-cyan)", title: "MCP marketplace", body: "Discover and install Model Context Protocol servers from inside the TUI. Browse, preview, attach." },
];

export function CliFeatures() {
  return (
    <section className="relative py-20">
      <div className="mx-auto max-w-6xl px-6">
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {ITEMS.map((it, i) => (
            <motion.div
              key={it.title}
              initial={{ opacity: 0, y: 14 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true, margin: "-60px" }}
              transition={{ duration: 0.35, delay: i * 0.04 }}
              className="rounded-2xl border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/40 p-5 backdrop-blur"
            >
              <div
                className="flex h-9 w-9 items-center justify-center rounded-lg"
                style={{ background: `${it.color}1a`, color: it.color }}
              >
                <it.icon className="h-4 w-4" />
              </div>
              <h3 className="mt-4 text-sm font-semibold text-white">{it.title}</h3>
              <p className="mt-1.5 text-sm leading-relaxed text-[color:var(--color-text-dim)]">{it.body}</p>
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  );
}
