"use client";

import { motion } from "framer-motion";
import { Briefcase, HeartPulse, Landmark, ShieldCheck } from "lucide-react";

const USE_CASES = [
  {
    icon: Briefcase,
    color: "var(--color-violet)",
    title: "Legal",
    blurb:
      "Draft, redline, and search privileged documents without a cloud processor in the loop. Attorney-client material never leaves the device.",
    examples: ["NDAs and LOIs", "Discovery search", "Contract redlines"],
  },
  {
    icon: HeartPulse,
    color: "var(--color-pink)",
    title: "Healthcare",
    blurb:
      "Summarize charts, draft notes, and pattern-match across PHI on a workstation you already control. No third-party processor, no BAA gymnastics.",
    examples: ["Chart summaries", "Intake drafting", "Coding assistance"],
  },
  {
    icon: Landmark,
    color: "var(--color-cyan)",
    title: "Finance & compliance",
    blurb:
      "Reconcile statements, parse filings, and prep audit packets against material that can&rsquo;t cross a network boundary. Logs stay in your transcripts directory.",
    examples: ["Audit prep", "Filing review", "Internal reconciliations"],
  },
];

export function Regulated() {
  return (
    <section id="regulated" className="relative py-28">
      <div
        aria-hidden
        className="absolute inset-0 -z-10"
        style={{
          background:
            "radial-gradient(900px 400px at 50% 0%, rgba(124,60,255,0.10), transparent 60%)",
        }}
      />
      <div className="mx-auto max-w-6xl px-6">
        <div className="mx-auto max-w-2xl text-center">
          <div className="inline-flex items-center gap-2 rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/50 px-3 py-1 text-xs text-[color:var(--color-text-dim)] backdrop-blur">
            <ShieldCheck className="h-3 w-3 text-[color:var(--color-cyan)]" />
            For regulated work
          </div>
          <h2 className="mt-4 text-balance text-4xl font-semibold tracking-tight sm:text-5xl">
            <span className="text-white/85">When the data</span>{" "}
            <span className="gradient-text">can&rsquo;t leave.</span>
          </h2>
          <p className="mt-4 text-pretty text-[color:var(--color-text-dim)]">
            Local execution turns a hard compliance problem into a non-problem.
            Vibn runs the model, the tools, and the transcripts on the same
            machine you already trust with the file. Nothing goes to a SaaS
            inference provider, because there isn&rsquo;t one.
          </p>
        </div>

        <div className="mt-14 grid gap-4 md:grid-cols-3">
          {USE_CASES.map((c, i) => (
            <motion.div
              key={c.title}
              initial={{ opacity: 0, y: 14 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true, margin: "-60px" }}
              transition={{ duration: 0.75, delay: i * 0.08, ease: [0.22, 1, 0.36, 1] }}
              className="relative overflow-hidden rounded-2xl border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/40 p-6 backdrop-blur transition hover:border-[color:var(--color-violet)]/40"
            >
              <div
                aria-hidden
                className="pointer-events-none absolute -right-12 -top-12 h-40 w-40 rounded-full opacity-30 blur-3xl"
                style={{ background: c.color }}
              />
              <div
                className="flex h-10 w-10 items-center justify-center rounded-lg"
                style={{ background: `${c.color}1a`, color: c.color }}
              >
                <c.icon className="h-5 w-5" />
              </div>
              <h3 className="mt-4 text-base font-semibold text-white">
                {c.title}
              </h3>
              <p className="mt-1.5 text-sm leading-relaxed text-[color:var(--color-text-dim)]">
                {c.blurb}
              </p>
              <ul className="mt-4 space-y-1.5 text-xs text-white/75">
                {c.examples.map((e) => (
                  <li key={e} className="flex items-center gap-2">
                    <span
                      className="h-1 w-1 rounded-full"
                      style={{ background: c.color }}
                    />
                    {e}
                  </li>
                ))}
              </ul>
            </motion.div>
          ))}
        </div>

        <p className="mx-auto mt-10 max-w-3xl text-center text-xs text-[color:var(--color-text-dim)]">
          Vibn doesn&rsquo;t certify HIPAA, SOC 2, or any regulation on your
          behalf. It removes the part of the stack that usually breaks them:
          remote inference. Your IT and compliance teams own the device; Vibn
          inherits whatever controls already protect it.
        </p>
      </div>
    </section>
  );
}
