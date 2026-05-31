"use client";

import { useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { ArrowRight, Copy, Check } from "lucide-react";

const LOG_LINES = [
  { c: "var(--color-text-dim)", t: "$ vibn" },
  { c: "var(--color-violet)", t: "vibn 0.4.0 · qwen2.5-coder:7b · context 32k" },
  { c: "var(--color-text-dim)", t: "loading observations from ~/.vibn/observations/projects/vibn …" },
  { c: "var(--color-green)", t: "✓ ready · 4 MCP servers connected" },
  { c: "var(--color-cyan)", t: "> find where we parse tool calls" },
  { c: "var(--color-violet)", t: "⏺ grep { pattern: \"parse_tool_calls\" }" },
  { c: "var(--color-text-dim)", t: "  → vibn-core/src/lib.rs:842" },
  { c: "var(--color-violet)", t: "⏺ read_file { path: \"vibn-core/src/lib.rs\", offset: 830, limit: 80 }" },
  { c: "var(--color-text-dim)", t: "  → 1 match · 14 lines" },
  { c: "var(--color-amber)", t: "★ parse_tool_calls_from_text() lives at vibn-core/src/lib.rs:842." },
  { c: "var(--color-amber)", t: "  It intercepts text-shaped JSON tool calls from smaller Ollama" },
  { c: "var(--color-amber)", t: "  models so you don't lose them when the model can't emit native blocks." },
];

const INSTALL_CMD = "cargo install vibn";

export function CliHero() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [visibleLines, setVisibleLines] = useState(0);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let timeout: ReturnType<typeof setTimeout>;
    const tick = (n: number) => {
      if (cancelled) return;
      if (n >= LOG_LINES.length) {
        timeout = setTimeout(() => {
          setVisibleLines(0);
          tick(0);
        }, 3800);
        return;
      }
      timeout = setTimeout(() => {
        setVisibleLines(n + 1);
        tick(n + 1);
      }, 600);
    };
    tick(0);
    return () => {
      cancelled = true;
      clearTimeout(timeout);
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let raf = 0;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const N = reduce ? 0 : 55;
    let mouse = { x: -9999, y: -9999 };

    const fit = () => {
      const r = canvas.getBoundingClientRect();
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      canvas.width = r.width * dpr;
      canvas.height = r.height * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(canvas);

    const colors = ["#22d3ee", "#a78bfa", "#34d399"];
    const ps = Array.from({ length: N }, () => ({
      x: Math.random(),
      y: Math.random(),
      vx: (Math.random() - 0.5) * 0.0006,
      vy: (Math.random() - 0.5) * 0.0006,
      r: 0.9 + Math.random() * 1.4,
      c: colors[Math.floor(Math.random() * colors.length)],
    }));

    const onMove = (e: MouseEvent) => {
      const r = canvas.getBoundingClientRect();
      mouse.x = (e.clientX - r.left) / r.width;
      mouse.y = (e.clientY - r.top) / r.height;
    };
    window.addEventListener("mousemove", onMove);

    const draw = () => {
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      ctx.clearRect(0, 0, w, h);

      for (let i = 0; i < ps.length; i++) {
        for (let j = i + 1; j < ps.length; j++) {
          const a = ps[i];
          const b = ps[j];
          const dx = (a.x - b.x) * w;
          const dy = (a.y - b.y) * h;
          const d2 = dx * dx + dy * dy;
          if (d2 < 130 * 130) {
            ctx.strokeStyle = `rgba(167,139,250,${0.10 * (1 - d2 / (130 * 130))})`;
            ctx.lineWidth = 0.5;
            ctx.beginPath();
            ctx.moveTo(a.x * w, a.y * h);
            ctx.lineTo(b.x * w, b.y * h);
            ctx.stroke();
          }
        }
      }
      for (const p of ps) {
        // Subtle attraction to mouse
        const dx = mouse.x - p.x;
        const dy = mouse.y - p.y;
        const d2 = dx * dx + dy * dy;
        if (d2 < 0.04) {
          p.vx += dx * 0.000004;
          p.vy += dy * 0.000004;
        }
        p.x += p.vx;
        p.y += p.vy;
        if (p.x < 0) p.x = 1;
        if (p.x > 1) p.x = 0;
        if (p.y < 0) p.y = 1;
        if (p.y > 1) p.y = 0;
        ctx.fillStyle = p.c;
        ctx.globalAlpha = 0.75;
        ctx.beginPath();
        ctx.arc(p.x * w, p.y * h, p.r, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.globalAlpha = 1;
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      window.removeEventListener("mousemove", onMove);
    };
  }, []);

  const copy = async () => {
    await navigator.clipboard.writeText(INSTALL_CMD);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <section className="relative isolate overflow-hidden pt-28 pb-20">
      <canvas ref={canvasRef} className="absolute inset-0 -z-10 h-full w-full" aria-hidden />
      <div
        aria-hidden
        className="absolute -top-40 left-1/2 h-[900px] w-[900px] -translate-x-1/2 rounded-full blur-3xl -z-10"
        style={{ background: "radial-gradient(circle, rgba(34,211,238,0.10), transparent 65%)" }}
      />
      <div
        aria-hidden
        className="absolute bottom-0 right-0 h-[600px] w-[600px] rounded-full blur-3xl -z-10"
        style={{ background: "radial-gradient(circle, rgba(167,139,250,0.10), transparent 65%)" }}
      />

      <div className="mx-auto max-w-5xl px-6 text-center">
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5 }}
          className="inline-flex items-center gap-2 rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/60 px-3 py-1 text-xs text-[color:var(--color-text-dim)] backdrop-blur"
        >
          <span className="font-mono text-[color:var(--color-cyan)]">$</span>
          Vibn CLI · Rust + Ratatui
        </motion.div>

        <h1 className="mt-5 text-balance text-5xl font-semibold leading-[1.02] tracking-tight sm:text-6xl lg:text-[72px]">
          <span className="gradient-text">Your terminal,</span>
          <br />
          <span className="text-white/90">but with agency.</span>
        </h1>

        <p className="mx-auto mt-6 max-w-xl text-pretty text-lg text-[color:var(--color-text-dim)]">
          The same Vibn engine, in a fullscreen TUI. Slash commands, model picker, marketplace, MCP browser — all rendered with Ratatui, all running on your machine.
        </p>

        <div className="mx-auto mt-8 flex max-w-md items-center gap-2 rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/60 px-4 py-2 backdrop-blur">
          <span className="font-mono text-sm text-[color:var(--color-violet)]">$</span>
          <span className="flex-1 truncate text-left font-mono text-sm text-white">{INSTALL_CMD}</span>
          <button
            onClick={copy}
            className="rounded-full p-1.5 text-[color:var(--color-text-dim)] transition hover:bg-white/5 hover:text-white"
            aria-label="Copy install command"
          >
            {copied ? <Check className="h-4 w-4 text-[color:var(--color-green)]" /> : <Copy className="h-4 w-4" />}
          </button>
        </div>

        <div className="mt-4 flex items-center justify-center gap-3 text-xs text-[color:var(--color-text-dim)]">
          <a href="#install" className="inline-flex items-center gap-1 hover:text-white">
            Other install methods <ArrowRight className="h-3 w-3" />
          </a>
        </div>

        {/* Terminal frame */}
        <motion.div
          initial={{ opacity: 0, y: 24, rotateX: 8 }}
          animate={{ opacity: 1, y: 0, rotateX: 0 }}
          transition={{ duration: 0.7, delay: 0.2 }}
          className="mx-auto mt-14 max-w-3xl text-left"
        >
          <div className="overflow-hidden rounded-2xl border border-[color:var(--color-border)] bg-[#07060d] shadow-2xl ring-glow">
            <div className="flex items-center gap-2 border-b border-[color:var(--color-border)] bg-[color:var(--color-surface)]/70 px-3 py-2">
              <span className="h-2.5 w-2.5 rounded-full bg-red-400/80" />
              <span className="h-2.5 w-2.5 rounded-full bg-amber/80" />
              <span className="h-2.5 w-2.5 rounded-full" style={{ background: "var(--color-green)" }} />
              <span className="ml-2 font-mono text-[11px] text-[color:var(--color-text-dim)]">vibn ~/projects/vibn</span>
            </div>
            <div className="relative h-[360px] overflow-hidden p-4 font-mono text-[12px] leading-relaxed">
              {LOG_LINES.slice(0, visibleLines).map((l, i) => (
                <motion.div
                  key={i}
                  initial={{ opacity: 0, x: -6 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ duration: 0.18 }}
                  style={{ color: l.c as string }}
                >
                  {l.t}
                </motion.div>
              ))}
              {visibleLines < LOG_LINES.length && (
                <span className="inline-block h-3 w-1.5 translate-y-[2px] bg-white anim-caret" />
              )}
            </div>
          </div>
        </motion.div>
      </div>
    </section>
  );
}
