"use client";

import { motion } from "framer-motion";

const BUILT_IN = [
  "read_file", "write_file", "edit", "grep", "glob", "list_dir",
  "shell", "plan", "task", "fetch", "generate_image", "read_image",
];

const MCP = [
  { name: "Linear", color: "#5e6ad2" },
  { name: "GitHub", color: "#8b949e" },
  { name: "Stripe", color: "#635bff" },
  { name: "Sentry", color: "#a737b4" },
  { name: "Vercel", color: "#ffffff" },
  { name: "Chrome DevTools", color: "#fbbc05" },
  { name: "Playwright", color: "#2ead33" },
  { name: "Plaid", color: "#00b3a4" },
];

export function Tools() {
  return (
    <section id="tools" className="relative py-28">
      <div className="mx-auto max-w-6xl px-6">
        <div className="grid items-start gap-12 lg:grid-cols-2">
          <div>
            <div className="inline-flex rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/50 px-3 py-1 text-xs text-[color:var(--color-text-dim)] backdrop-blur">
              Tools & MCP
            </div>
            <h2 className="mt-4 text-balance text-4xl font-semibold tracking-tight sm:text-5xl">
              <span className="text-white/85">Hand it the keys.</span>{" "}
              <span className="gradient-text">Carefully.</span>
            </h2>
            <p className="mt-4 max-w-md text-pretty text-[color:var(--color-text-dim)]">
              Vibn ships with a battle-tested built-in toolset and speaks the Model Context Protocol fluently. Every tool call runs on your machine against your files &mdash; nothing crosses the network unless you wire an MCP server that explicitly does. Permission checks gate every call.
            </p>

            <div className="mt-8 rounded-2xl border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/40 p-4 font-mono text-xs">
              <div className="text-[color:var(--color-text-dim)]">~/.vibn/config.json</div>
              <pre className="mt-2 overflow-x-auto leading-relaxed text-white/85">{`{
  "permissions": {
    "shell": "ask",
    "edit": "allow",
    "fetch": "deny"
  },
  "mcp_servers": {
    "linear": { "command": "npx",
                "args": ["-y", "@linear/mcp"] }
  }
}`}</pre>
            </div>
          </div>

          <div className="space-y-6">
            <div>
              <div className="mb-3 text-xs uppercase tracking-wider text-[color:var(--color-text-dim)]">
                Built-in tools
              </div>
              <div className="flex flex-wrap gap-2">
                {BUILT_IN.map((t, i) => (
                  <motion.span
                    key={t}
                    initial={{ opacity: 0, scale: 0.92 }}
                    whileInView={{ opacity: 1, scale: 1 }}
                    viewport={{ once: true }}
                    transition={{ duration: 0.55, delay: i * 0.05, ease: [0.22, 1, 0.36, 1] }}
                    className="rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/60 px-3 py-1 font-mono text-xs text-white/90"
                  >
                    {t}
                  </motion.span>
                ))}
              </div>
            </div>

            <div>
              <div className="mb-3 text-xs uppercase tracking-wider text-[color:var(--color-text-dim)]">
                MCP servers (any compatible one works)
              </div>
              <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                {MCP.map((m, i) => (
                  <motion.div
                    key={m.name}
                    initial={{ opacity: 0, y: 12 }}
                    whileInView={{ opacity: 1, y: 0 }}
                    viewport={{ once: true }}
                    transition={{ duration: 0.6, delay: i * 0.05, ease: [0.22, 1, 0.36, 1] }}
                    className="flex items-center gap-2 rounded-xl border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/40 px-3 py-2 text-sm text-white/85 transition hover:border-[color:var(--color-violet)]/40"
                  >
                    <span className="h-2 w-2 rounded-full" style={{ background: m.color }} />
                    {m.name}
                  </motion.div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
