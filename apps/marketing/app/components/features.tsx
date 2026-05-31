"use client";

import { motion } from "framer-motion";
import {
  Brain,
  Cable,
  CloudOff,
  FileCode2,
  Lock,
  Plug,
  ShieldCheck,
  Terminal,
} from "lucide-react";

const FEATURES = [
  {
    icon: CloudOff,
    color: "var(--color-cyan)",
    title: "Runs entirely on your machine",
    body: "Vibn pairs with Ollama on your own hardware. No accounts, no cloud round-trips, no quotas — every prompt, file, and answer stays on the device.",
  },
  {
    icon: Brain,
    color: "var(--color-violet)",
    title: "Bring your own model",
    body: "Qwen, DeepSeek, Llama, Mistral, GPT-OSS — any model Ollama can run. Pick a coder for code, a generalist for prose, a tiny one for fast triage. Switch in one click.",
  },
  {
    icon: Terminal,
    color: "var(--color-green)",
    title: "Real tools, not just chat",
    body: "Read files, edit, search, run commands, generate images, browse — a built-in toolset that touches your actual filesystem, with explicit per-call permission checks.",
  },
  {
    icon: Plug,
    color: "var(--color-pink)",
    title: "MCP protocol native",
    body: "Speak the Model Context Protocol fluently. Wire up Linear, GitHub, your firm's internal MCP server, or any compatible one — and only the ones you trust.",
  },
  {
    icon: FileCode2,
    color: "var(--color-amber)",
    title: "Project-aware memory",
    body: "Per-project observations build up over time. Vibn remembers conventions, sensitivities, and decisions per workspace so it doesn't drag stale context across matters.",
  },
  {
    icon: ShieldCheck,
    color: "var(--color-cyan)",
    title: "Permission-checked tool calls",
    body: "Every shell command, every edit, every external call passes through configurable allow / ask / deny rules. Audit what touched what, after the fact.",
  },
  {
    icon: Cable,
    color: "var(--color-violet)",
    title: "Hooks for everything",
    body: "Fire shell commands on session start, before / after edits, on compaction, on commands. Wire Vibn into your existing controls — DLP, backup, audit logging.",
  },
  {
    icon: Lock,
    color: "var(--color-green)",
    title: "Transcripts you own",
    body: "Sessions land as JSONL in ~/.vibn/transcripts/. Keep them, ship them through your retention policy, or throw them away. There is no cloud copy.",
  },
];

export function Features() {
  return (
    <section id="features" className="relative py-28">
      <div className="mx-auto max-w-6xl px-6">
        <div className="mx-auto max-w-2xl text-center">
          <div className="inline-flex rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/50 px-3 py-1 text-xs text-[color:var(--color-text-dim)] backdrop-blur">
            Features
          </div>
          <h2 className="mt-4 text-balance text-4xl font-semibold tracking-tight sm:text-5xl">
            <span className="gradient-text">A real agent.</span>{" "}
            <span className="text-white/85">On your machine.</span>
          </h2>
          <p className="mt-4 text-pretty text-[color:var(--color-text-dim)]">
            Built in Rust. Powered by Ollama. Designed so you can hand it serious work — and trust where the data goes.
          </p>
        </div>

        <div className="mt-16 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {FEATURES.map((f, i) => (
            <motion.div
              key={f.title}
              initial={{ opacity: 0, y: 14 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true, margin: "-80px" }}
              transition={{ duration: 0.7, delay: i * 0.06, ease: [0.22, 1, 0.36, 1] }}
              className="group relative overflow-hidden rounded-2xl border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/40 p-5 backdrop-blur transition hover:border-[color:var(--color-violet)]/40 hover:bg-[color:var(--color-surface-2)]/60"
            >
              <div
                className="absolute -inset-px -z-10 opacity-0 transition-opacity duration-500 group-hover:opacity-100"
                style={{ background: `radial-gradient(400px circle at var(--mx,50%) var(--my,50%), ${f.color}22, transparent 40%)` }}
              />
              <div
                className="flex h-9 w-9 items-center justify-center rounded-lg"
                style={{ background: `${f.color}1a`, color: f.color }}
              >
                <f.icon className="h-4 w-4" />
              </div>
              <h3 className="mt-4 text-sm font-semibold text-white">{f.title}</h3>
              <p className="mt-1.5 text-sm leading-relaxed text-[color:var(--color-text-dim)]">{f.body}</p>
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  );
}
