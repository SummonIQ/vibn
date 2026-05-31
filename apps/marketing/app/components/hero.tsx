"use client";

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { ArrowRight, Cpu, Download, Sparkles } from "lucide-react";
import Link from "next/link";
import { Logo } from "./logo";
import { Button } from "./ui/button";

const TASKS = [
  "ship a feature",
  "redline this NDA",
  "summarize these patient notes",
  "reconcile this spreadsheet",
  "draft a privileged memo",
  "refactor that module",
  "review this contract",
  "explain this codebase",
];

type Scenario = {
  title: string;
  projectPath: string;
  context: string;
  model: string;
  sidebarTime: string;
  prompt: string;
  tool: {
    name: string;
    arg: string;
    resultBadge: string;
    resultKind: "text" | "image" | "linear" | "diff";
    resultText?: string;
    imageGradient?: string;
    imageLabel?: string;
    diffLines?: { kind: "add" | "del"; text: string }[];
    linearTicket?: { id: string; title: string };
  };
  answer: { lead: React.ReactNode; sub?: React.ReactNode };
};

const SCENARIOS: Scenario[] = [
  {
    title: "Refactor agent loop",
    projectPath: "~/projects/vibn",
    context: "14k / 32k",
    model: "qwen2.5-coder:7b",
    sidebarTime: "now",
    prompt: "Find where we parse tool calls for smaller models",
    tool: {
      name: "grep",
      arg: "pattern: parse_tool_calls",
      resultBadge: "\u2713 1 match",
      resultKind: "text",
      resultText: "vibn-core/src/lib.rs:842 \u2014 fn parse_tool_calls_from_text(input: &str)",
    },
    answer: {
      lead: (
        <p>
          <span className="font-mono text-[color:var(--color-brand-2)]">parse_tool_calls_from_text()</span> lives at{" "}
          <span className="rounded bg-[color:var(--color-surface-2)] px-1 py-0.5 font-mono text-[11px] text-white/80">vibn-core/src/lib.rs:842</span>.
        </p>
      ),
      sub: <>It catches JSON-shaped tool calls from smaller Ollama models that can&rsquo;t emit native tool blocks.</>,
    },
  },
  {
    title: "Redline acquisition LOI",
    projectPath: "~/deals/acme-loi",
    context: "8k / 32k",
    model: "mistral-small:24b",
    sidebarTime: "1h",
    prompt: "Redline this draft toward buyer-favorable terms",
    tool: {
      name: "read_file",
      arg: "path: loi-draft.md",
      resultBadge: "\u2713 4 proposed edits",
      resultKind: "diff",
      diffLines: [
        { kind: "del", text: "Seller&rsquo;s reps survive 24 months." },
        { kind: "add", text: "Seller&rsquo;s reps survive 12 months; materiality scrape." },
        { kind: "del", text: "Exclusivity period: 60 days." },
        { kind: "add", text: "Exclusivity period: 30 days." },
      ],
    },
    answer: {
      lead: <p>Drafted four edits, all narrowing buyer-side risk.</p>,
      sub: <>Tightened survival, shortened exclusivity, scrubbed catch-all confidentiality. Nothing left the machine.</>,
    },
  },
  {
    title: "Hero illustration",
    projectPath: "~/projects/marketing",
    context: "3k / 32k",
    model: "flux-schnell (local)",
    sidebarTime: "3h",
    prompt: "Generate a hero illustration for the onboarding screen",
    tool: {
      name: "generate_image",
      arg: "prompt: minimalist isometric workspace, violet light",
      resultBadge: "\u2713 1024\u00d71024",
      resultKind: "image",
      imageGradient:
        "linear-gradient(135deg, rgba(167,139,250,0.85), rgba(34,211,238,0.75) 55%, rgba(242,65,183,0.7))",
      imageLabel: "onboarding-hero.png",
    },
    answer: {
      lead: (
        <p>
          Saved to <span className="rounded bg-[color:var(--color-surface-2)] px-1 py-0.5 font-mono text-[11px] text-white/80">assets/onboarding-hero.png</span>.
        </p>
      ),
      sub: <>Generated locally via ComfyUI. No third-party processor.</>,
    },
  },
  {
    title: "Patient intake summary",
    projectPath: "~/charts/2026-05-26",
    context: "5k / 32k",
    model: "llama3.2:3b",
    sidebarTime: "yesterday",
    prompt: "Summarize this intake for the cardiology referral",
    tool: {
      name: "read_file",
      arg: "path: intake.pdf",
      resultBadge: "\u2713 12 fields",
      resultKind: "text",
      resultText: "Hx \u00b7 meds \u00b7 vitals \u00b7 labs \u00b7 ECG findings parsed and structured.",
    },
    answer: {
      lead: <p>62yo M, AFib history, irregular ECG, stable vitals. Cardiology workup indicated.</p>,
      sub: <>Full structured note saved to the chart directory. PHI never left the laptop.</>,
    },
  },
  {
    title: "File the bug we just fixed",
    projectPath: "~/projects/vibn",
    context: "11k / 32k",
    model: "qwen2.5-coder:14b",
    sidebarTime: "3d",
    prompt: "Open a Linear ticket for the auth-loop fix we just shipped",
    tool: {
      name: "mcp__linear__create_issue",
      arg: "team: VBN, priority: high",
      resultBadge: "\u2713 VBN-414",
      resultKind: "linear",
      linearTicket: { id: "VBN-414", title: "auth-loop: drop stale session on token revoke" },
    },
    answer: {
      lead: (
        <p>
          Created <span className="rounded bg-[color:var(--color-surface-2)] px-1 py-0.5 font-mono text-[11px] text-white/80">VBN-414</span> in Linear with a link to the merge commit.
        </p>
      ),
      sub: <>MCP call ran with the per-tool permission you allowed at session start.</>,
    },
  },
];


const PER_CHAR_GRADIENT =
  "linear-gradient(135deg, rgba(255,255,255,0.15) 0%, rgba(0,0,0,0.09) 100%), linear-gradient(110deg, var(--color-brand-1), var(--color-brand-2) 55%, var(--color-brand-3))";

export function Hero() {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [taskIdx, setTaskIdx] = useState(0);
  const [typed, setTyped] = useState("");
  const [phase, setPhase] = useState<"typing" | "dwell" | "deleting">("typing");
  const [step, setStep] = useState(0);
  const [scenarioIdx, setScenarioIdx] = useState(0);
  const scenario = SCENARIOS[scenarioIdx];
  const typedRef = useRef<HTMLSpanElement>(null);
  const [typedWidth, setTypedWidth] = useState<number | null>(null);
  const [widthTransitionMs, setWidthTransitionMs] = useState(120);

  useLayoutEffect(() => {
    const el = typedRef.current;
    if (!el) return;
    const MIN = 240;
    const natural = el.scrollWidth;
    const target = Math.max(MIN, natural);
    setTypedWidth((prev) => {
      if (prev === null) {
        setWidthTransitionMs(0);
        return target;
      }
      if (target > prev) {
        // Growing: snappy, keep up with each keystroke.
        setWidthTransitionMs(90);
      } else if (target < prev) {
        // Shrinking: hang at the old width briefly, then ease back.
        setWidthTransitionMs(900);
      }
      return target;
    });
  }, [typed]);
  const [tilt, setTilt] = useState({ rx: 14, ry: 0 });
  useEffect(() => {
    const current = TASKS[taskIdx];
    let timeout: ReturnType<typeof setTimeout>;

    // Human-feeling cadence: ~150ms baseline ≈ 80 WPM with wide jitter and
    // occasional longer "thinking" pauses so the rhythm reads as a person typing.
    const typeDelay = () => {
      const base = 155;
      const jitter = (Math.random() - 0.5) * 110;
      const microPause = Math.random() < 0.1 ? 260 : 0;
      const longPause = Math.random() < 0.03 ? 520 : 0;
      return Math.max(70, base + jitter + microPause + longPause);
    };
    const deleteDelay = () => 50 + (Math.random() - 0.5) * 30;

    if (phase === "typing") {
      if (typed.length < current.length) {
        timeout = setTimeout(() => {
          setTyped(current.slice(0, typed.length + 1));
        }, typeDelay());
      } else {
        timeout = setTimeout(() => setPhase("dwell"), 0);
      }
    } else if (phase === "dwell") {
      timeout = setTimeout(() => setPhase("deleting"), 4000);
    } else if (phase === "deleting") {
      if (typed.length > 0) {
        timeout = setTimeout(() => {
          setTyped(typed.slice(0, -1));
        }, deleteDelay());
      } else {
        timeout = setTimeout(() => {
          setTaskIdx((i) => (i + 1) % TASKS.length);
          setPhase("typing");
        }, 650);
      }
    }

    return () => clearTimeout(timeout);
  }, [typed, phase, taskIdx]);

  useEffect(() => {
    let cancelled = false;
    let timeout: ReturnType<typeof setTimeout>;
    const STEPS = 6;
    const tick = (n: number) => {
      if (cancelled) return;
      if (n >= STEPS) {
        timeout = setTimeout(() => {
          setScenarioIdx((i) => (i + 1) % SCENARIOS.length);
          setStep(0);
          tick(0);
        }, 4000);
        return;
      }
      timeout = setTimeout(() => {
        setStep(n + 1);
        tick(n + 1);
      }, 950);
    };
    tick(0);
    return () => {
      cancelled = true;
      clearTimeout(timeout);
    };
  }, []);

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    let raf = 0;
    let targetX = 0,
      targetY = 0,
      curX = 0,
      curY = 0;
    const onMove = (e: MouseEvent) => {
      const r = wrap.getBoundingClientRect();
      const cx = r.left + r.width / 2;
      const cy = r.top + r.height / 2;
      const dx = (e.clientX - cx) / r.width;
      const dy = (e.clientY - cy) / r.height;
      targetX = Math.max(-1, Math.min(1, dx)) * 8;
      targetY = Math.max(-1, Math.min(1, dy)) * 8;
    };
    const tick = () => {
      curX += (targetX - curX) * 0.08;
      curY += (targetY - curY) * 0.08;
      setTilt({ rx: 14 - curY, ry: curX });
      raf = requestAnimationFrame(tick);
    };
    window.addEventListener("mousemove", onMove);
    raf = requestAnimationFrame(tick);
    return () => {
      window.removeEventListener("mousemove", onMove);
      cancelAnimationFrame(raf);
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    let raf = 0;

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

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const N = reduce ? 0 : 38;
    const particles = Array.from({ length: N }, () => {
      const colors = ["#22d3ee", "#a78bfa", "#34d399", "#f472b6"];
      return {
        x: Math.random(),
        y: Math.random(),
        vx: (Math.random() - 0.5) * 0.0008,
        vy: (Math.random() - 0.5) * 0.0008,
        r: 1 + Math.random() * 1.4,
        c: colors[Math.floor(Math.random() * colors.length)],
        a: 0.5 + Math.random() * 0.5,
      };
    });

    const draw = () => {
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      ctx.clearRect(0, 0, w, h);

      // Connection lines (only between nearby particles)
      for (let i = 0; i < particles.length; i++) {
        for (let j = i + 1; j < particles.length; j++) {
          const a = particles[i];
          const b = particles[j];
          const dx = (a.x - b.x) * w;
          const dy = (a.y - b.y) * h;
          const d2 = dx * dx + dy * dy;
          if (d2 < 110 * 110) {
            const alpha = 0.12 * (1 - d2 / (110 * 110));
            ctx.strokeStyle = `rgba(167,139,250,${alpha})`;
            ctx.lineWidth = 0.6;
            ctx.beginPath();
            ctx.moveTo(a.x * w, a.y * h);
            ctx.lineTo(b.x * w, b.y * h);
            ctx.stroke();
          }
        }
      }

      for (const p of particles) {
        p.x += p.vx;
        p.y += p.vy;
        // Bounce inside screen: enforces the "data stays local" metaphor
        if (p.x < 0.01 || p.x > 0.99) p.vx *= -1;
        if (p.y < 0.01 || p.y > 0.99) p.vy *= -1;
        ctx.fillStyle = p.c;
        ctx.globalAlpha = p.a;
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
    };
  }, []);

  return (
    <section className="relative isolate overflow-hidden pt-28 pb-24 sm:pt-32 sm:pb-32">
      {/* Ambient background blobs */}
      <div className="pointer-events-none absolute inset-0 -z-10">
        <div className="absolute left-[10%] top-[10%] h-[420px] w-[420px] rounded-full bg-violet/25 blur-[120px] anim-drift-1" style={{ background: "radial-gradient(circle, rgba(167,139,250,0.45), transparent 60%)" }} />
        <div className="absolute right-[8%] top-[20%] h-[460px] w-[460px] rounded-full blur-[120px] anim-drift-2" style={{ background: "radial-gradient(circle, rgba(34,211,238,0.35), transparent 60%)" }} />
        <div className="absolute left-[40%] bottom-[5%] h-[380px] w-[380px] rounded-full blur-[120px] anim-drift-3" style={{ background: "radial-gradient(circle, rgba(52,211,153,0.20), transparent 60%)" }} />
        <div className="absolute inset-0 dot-grid opacity-60" />
      </div>

      <div className="mx-auto max-w-7xl px-6">
        <div className="flex flex-col items-center text-center">
          {/* Text */}
          <div className="relative w-full">
            <motion.div
              initial={{ opacity: 0, y: 16 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.6 }}
              className="inline-flex items-center gap-2 rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/60 px-3 py-1 text-xs text-[color:var(--color-text-dim)] backdrop-blur"
            >
              <span className="relative flex h-1.5 w-1.5">
                <span className="absolute inline-flex h-full w-full rounded-full bg-green opacity-75 anim-pulse-dot" />
                <span className="relative inline-flex h-1.5 w-1.5 rounded-full" style={{ background: "var(--color-green)" }} />
              </span>
              <span>100% local · your data never leaves the device</span>
            </motion.div>

            <h1 className="mt-5 text-balance text-4xl font-semibold leading-[1.05] tracking-tight sm:text-5xl lg:text-[60px]">
{/* Terminal-style container: square-ish, side-specific borders for grounded feel */}
              <span className="mt-5 flex justify-center">
                <span
                  className="relative inline-flex max-w-[98vw] items-center overflow-hidden whitespace-nowrap rounded-xl bg-[color:var(--color-surface)]/85 p-3 backdrop-blur-md sm:p-4"
                  style={{
                    border: "1px solid",
                    borderColor:
                      "rgba(255,255,255,0.06) transparent rgba(0,0,0,0.7) transparent",
                    boxShadow:
                      "inset 0 1px 0 rgba(255,255,255,0.04), inset 0 -1px 0 rgba(0,0,0,0.5), 0 14px 32px -10px rgba(0,0,0,0.65), 0 30px 60px -30px rgba(124,60,255,0.35)",
                  }}
                >
                  {/* Diagonal light-to-dark overlay across the whole container */}
                  <span
                    aria-hidden
                    className="pointer-events-none absolute inset-0 rounded-xl"
                    style={{
                      backgroundImage:
                        "linear-gradient(135deg, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0.04) 30%, rgba(255,255,255,0) 60%, rgba(0,0,0,0.13) 100%)",
                    }}
                  />
                  <span
                    className="relative font-mono text-[0.78em] font-bold leading-none whitespace-nowrap"
                    style={{ transform: "translateY(-4px)" }}
                    aria-live="polite"
                    aria-atomic="true"
                  >
                    <span
                      aria-hidden
                      className="mr-4 inline-block text-[color:var(--color-text-dim)]/70"
                    >
                      &gt;
                    </span>
                    <span
                      className="inline-block whitespace-nowrap pr-7 text-left align-bottom"
                      style={{
                        width:
                          typedWidth !== null ? `${typedWidth}px` : "auto",
                        transition: `width ${widthTransitionMs}ms cubic-bezier(0.22, 1, 0.36, 1)`,
                      }}
                    >
                      <span
                        ref={typedRef}
                        className="inline-block whitespace-nowrap text-left"
                      >
                        <span
                          style={{
                            backgroundImage: PER_CHAR_GRADIENT,
                            WebkitBackgroundClip: "text",
                            backgroundClip: "text",
                            color: "transparent",
                          }}
                        >
                          {typed}
                        </span>
                        <span
                          aria-hidden
                          className={`inline-block align-baseline anim-caret-grad ${
                            typed.length > 0 ? "ml-2" : ""
                          }`}
                          style={{
                            height: "calc(0.85em + 2px)",
                            width: "3px",
                            boxShadow:
                              "0 0 8px rgba(242,65,183,0.55), 0 0 16px rgba(124,60,255,0.35)",
                            transform: "translateY(calc(0.05em + 3px))",
                          }}
                        />
                      </span>
                    </span>
                  </span>
                </span>
              </span>
            </h1>

            <p className="mx-auto mt-6 max-w-xl text-pretty px-4 text-lg text-[color:var(--color-text-dim)]">
              A real AI agent that runs entirely on your machine. Pair with Ollama, hand it tools and MCP servers, and let it touch the work you can&rsquo;t send to the cloud &mdash; code, contracts, patient notes, financials, anything.
            </p>

            <div className="mt-8 flex flex-wrap items-center justify-center gap-3">
              <Button asChild className="group">
                <a href="#download">
                  <Download className="h-4 w-4" />
                  Download for Mac
                  <ArrowRight className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
                </a>
              </Button>
              <Button asChild variant="secondary">
                <Link href="/cli">
                  CLI version
                </Link>
              </Button>
            </div>

            <div className="mt-8 flex items-center justify-center gap-6 text-xs text-[color:var(--color-text-dim)]">
              <div className="flex items-center gap-2">
                <Cpu className="h-3.5 w-3.5 text-[color:var(--color-cyan)]" />
                Apple Silicon native
              </div>
              <div className="flex items-center gap-2">
                <Sparkles className="h-3.5 w-3.5 text-[color:var(--color-violet)]" />
                Bring your own model
              </div>
            </div>
          </div>

          {/* 3D Laptop with contained particles */}
          <div ref={wrapRef} className="relative mx-auto mt-16 w-full max-w-5xl" style={{ perspective: "1800px" }}>
            <motion.div
              initial={{ opacity: 0, y: 30, rotateX: 20 }}
              animate={{ opacity: 1, y: 0, rotateX: tilt.rx, rotateY: tilt.ry }}
              transition={{ opacity: { duration: 0.8 }, y: { duration: 0.8 } }}
              style={{ transformStyle: "preserve-3d", rotateX: tilt.rx, rotateY: tilt.ry }}
              className="relative"
            >
              {/* Laptop frame (lid) */}
              <div className="relative rounded-[20px] border border-[color:var(--color-border)] bg-gradient-to-b from-[#1c1b2d] to-[#0d0c1c] p-2.5 ring-glow">
                {/* Screen */}
                <div className="relative aspect-[16/10] overflow-hidden rounded-[12px] bg-[#07060d]">
                  {/* Particles confined inside the screen */}
                  <canvas
                    ref={canvasRef}
                    className="absolute inset-0 h-full w-full"
                    aria-hidden
                  />

                  {/* Subtle scanline */}
                  <div className="pointer-events-none absolute inset-x-0 h-px bg-gradient-to-r from-transparent via-[color:var(--color-violet)]/40 to-transparent anim-scan" />

                  {/* Notch */}
                  <div className="absolute left-1/2 top-0 z-10 flex h-5 -translate-x-1/2 items-center gap-1.5 rounded-b-lg bg-black px-3 text-[10px] font-mono text-[color:var(--color-green)]">
                    <span className="relative flex h-1.5 w-1.5">
                      <span className="absolute inline-flex h-full w-full rounded-full bg-green opacity-75 anim-pulse-dot" style={{ background: "var(--color-green)" }} />
                      <span className="relative inline-flex h-1.5 w-1.5 rounded-full" style={{ background: "var(--color-green)" }} />
                    </span>
                    running locally
                  </div>

                  {/* Desktop app GUI */}
                  <div className="absolute inset-0 flex flex-col bg-[color:var(--color-bg)]/95 text-left">
                    {/* Title bar with traffic lights */}
                    <div className="flex h-9 items-center gap-3 border-b border-[color:var(--color-border)]/70 bg-[color:var(--color-bg-2)]/80 px-4 backdrop-blur">
                      <div className="flex gap-1.5">
                        <span className="h-3 w-3 rounded-full bg-[#ff5f57]" />
                        <span className="h-3 w-3 rounded-full bg-[#febc2e]" />
                        <span className="h-3 w-3 rounded-full bg-[#28c840]" />
                      </div>
                      <div className="flex-1" />
                      <div className="flex items-center gap-2 text-[11px] text-[color:var(--color-text-dim)]">
                        <Logo className="h-3.5 w-3.5" />
                        <span className="font-medium text-white/85">vibn</span>
                        <span>·</span>
                        <span>{scenario.title}</span>
                      </div>
                      <div className="flex-1" />
                      <div className="h-1 w-1 rounded-full bg-[color:var(--color-text-dim)]/40" />
                    </div>

                    <div className="flex min-h-0 flex-1">
                      {/* Sidebar */}
                      <aside className="hidden w-44 shrink-0 flex-col gap-3 border-r border-[color:var(--color-border)]/70 bg-[color:var(--color-bg-2)]/40 p-3 sm:flex md:w-52">
                        <button className="flex items-center justify-between gap-2 rounded-lg border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/60 px-2.5 py-1.5 text-[11px] font-medium text-white/90">
                          <span className="flex items-center gap-1.5">
                            <span className="text-[color:var(--color-brand-2)]">+</span>
                            New chat
                          </span>
                          <span className="rounded bg-[color:var(--color-surface-2)] px-1 py-0.5 text-[9px] text-[color:var(--color-text-dim)]">⌘N</span>
                        </button>
                        <div className="space-y-0.5">
                          {SCENARIOS.map((s, i) => (
                            <div
                              key={s.title}
                              className={`flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-[11px] transition-colors ${
                                i === scenarioIdx
                                  ? "bg-[color:var(--color-surface)] text-white"
                                  : "text-[color:var(--color-text-dim)]"
                              }`}
                            >
                              <span className="truncate">{s.title}</span>
                              <span className="shrink-0 text-[9px] opacity-60">{s.sidebarTime}</span>
                            </div>
                          ))}
                        </div>
                        <div className="mt-auto space-y-1.5 pt-2">
                          <div className="flex items-center gap-2 rounded-md border border-[color:var(--color-border)]/60 bg-[color:var(--color-surface)]/40 px-2 py-1.5 text-[10px]">
                            <span className="h-1.5 w-1.5 rounded-full anim-pulse-dot" style={{ background: "var(--color-green)" }} />
                            <span className="truncate font-mono text-white/85">{scenario.model}</span>
                          </div>
                          <div className="flex items-center gap-2 rounded-md px-2 py-1 text-[10px] text-[color:var(--color-text-dim)]">
                            <span className="flex h-4 w-4 items-center justify-center rounded-full bg-gradient-to-br from-[color:var(--color-brand-1)] to-[color:var(--color-brand-3)] text-[8px] font-bold text-white">S</span>
                            Steven
                          </div>
                        </div>
                      </aside>

                      {/* Chat pane */}
                      <div className="flex min-h-0 flex-1 flex-col">
                        {/* Chat header */}
                        <div className="flex items-center justify-between gap-2 border-b border-[color:var(--color-border)]/40 px-4 py-2.5">
                          <div className="flex min-w-0 items-center gap-2">
                            <span className="truncate text-[12px] font-medium text-white/90">{scenario.title}</span>
                            <span className="rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/60 px-1.5 py-0.5 text-[9px] text-[color:var(--color-text-dim)]">
                              {scenario.projectPath}
                            </span>
                          </div>
                          <div className="flex items-center gap-1.5 text-[10px] text-[color:var(--color-text-dim)]">
                            <span className="rounded-md border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/60 px-1.5 py-0.5 font-mono">{scenario.context}</span>
                          </div>
                        </div>

                        {/* Messages */}
                        <div className="min-h-0 flex-1 space-y-3 overflow-hidden px-4 py-3 text-[12px]">
                          {/* User bubble */}
                          {step >= 1 && (
                            <motion.div
                              key={`prompt-${scenarioIdx}`}
                              initial={{ opacity: 0, y: 6 }}
                              animate={{ opacity: 1, y: 0 }}
                              transition={{ duration: 0.25 }}
                              className="flex justify-end"
                            >
                              <div className="max-w-[80%] rounded-2xl rounded-br-md bg-gradient-to-br from-[color:var(--color-brand-1)]/85 to-[color:var(--color-brand-2)]/75 px-3.5 py-2 text-[color:var(--color-bg)] shadow-lg shadow-[color:var(--color-brand-1)]/20">
                                {scenario.prompt}
                              </div>
                            </motion.div>
                          )}

                          {/* Assistant header */}
                          {step >= 2 && (
                            <motion.div
                              key={`think-${scenarioIdx}`}
                              initial={{ opacity: 0, y: 6 }}
                              animate={{ opacity: 1, y: 0 }}
                              className="flex items-center gap-2 text-[10px] uppercase tracking-wider text-[color:var(--color-text-dim)]"
                            >
                              <Logo className="h-3.5 w-3.5" />
                              <span>vibn</span>
                              <span className="h-1 w-1 rounded-full bg-[color:var(--color-text-dim)]/50" />
                              <span>thinking\u2026</span>
                            </motion.div>
                          )}

                          {/* Tool call card */}
                          {step >= 3 && (
                            <motion.div
                              key={`tool-${scenarioIdx}`}
                              initial={{ opacity: 0, y: 6 }}
                              animate={{ opacity: 1, y: 0 }}
                              transition={{ duration: 0.25 }}
                              className="overflow-hidden rounded-xl border border-[color:var(--color-border)]/80 bg-[color:var(--color-surface)]/50 font-mono backdrop-blur"
                            >
                              <div className="flex items-center justify-between gap-2 border-b border-[color:var(--color-border)]/60 bg-[color:var(--color-surface-2)]/40 px-3 py-1.5 text-[10px]">
                                <div className="flex min-w-0 items-center gap-1.5">
                                  <span className="text-[color:var(--color-brand-2)]">\u23fa</span>
                                  <span className="text-white/85">{scenario.tool.name}</span>
                                  <span className="truncate text-[color:var(--color-text-dim)]">{scenario.tool.arg}</span>
                                </div>
                                {step >= 4 && (
                                  <span className="shrink-0" style={{ color: "var(--color-green)" }}>
                                    {scenario.tool.resultBadge}
                                  </span>
                                )}
                              </div>
                              {step >= 4 && (
                                <div className="px-3 py-2 text-[10px] text-[color:var(--color-text-dim)]">
                                  {scenario.tool.resultKind === "text" && (
                                    <span>{scenario.tool.resultText}</span>
                                  )}
                                  {scenario.tool.resultKind === "image" && (
                                    <div className="flex items-center gap-3">
                                      <div
                                        className="h-16 w-16 shrink-0 overflow-hidden rounded-md ring-1 ring-white/10"
                                        style={{ backgroundImage: scenario.tool.imageGradient }}
                                      >
                                        <div className="h-full w-full mix-blend-overlay" style={{ background: "radial-gradient(circle at 30% 20%, rgba(255,255,255,0.5), transparent 60%)" }} />
                                      </div>
                                      <div className="min-w-0">
                                        <div className="truncate font-mono text-white/85">{scenario.tool.imageLabel}</div>
                                        <div className="mt-0.5 text-[9px] uppercase tracking-wider text-[color:var(--color-text-dim)]">local diffusion \u00b7 no upload</div>
                                      </div>
                                    </div>
                                  )}
                                  {scenario.tool.resultKind === "diff" && (
                                    <div className="space-y-0.5 font-mono text-[10px]">
                                      {scenario.tool.diffLines?.map((d, i) => (
                                        <div
                                          key={i}
                                          className={
                                            d.kind === "add"
                                              ? "text-[color:var(--color-green)]"
                                              : "text-[color:var(--color-pink)] line-through opacity-70"
                                          }
                                        >
                                          {d.kind === "add" ? "+ " : "- "}
                                          <span dangerouslySetInnerHTML={{ __html: d.text }} />
                                        </div>
                                      ))}
                                    </div>
                                  )}
                                  {scenario.tool.resultKind === "linear" && scenario.tool.linearTicket && (
                                    <div className="flex items-center gap-2">
                                      <span className="rounded bg-[color:var(--color-violet)]/15 px-1.5 py-0.5 font-mono text-[10px] text-[color:var(--color-violet)]">
                                        {scenario.tool.linearTicket.id}
                                      </span>
                                      <span className="truncate text-white/80">{scenario.tool.linearTicket.title}</span>
                                    </div>
                                  )}
                                </div>
                              )}
                            </motion.div>
                          )}

                          {/* Assistant final answer */}
                          {step >= 5 && (
                            <motion.div
                              key={`answer-${scenarioIdx}`}
                              initial={{ opacity: 0, y: 6 }}
                              animate={{ opacity: 1, y: 0 }}
                              transition={{ duration: 0.25 }}
                              className="text-white/90"
                            >
                              {scenario.answer.lead}
                              {scenario.answer.sub && (
                                <p className="mt-1.5 text-[color:var(--color-text-dim)]">
                                  {scenario.answer.sub}
                                </p>
                              )}
                            </motion.div>
                          )}
                        </div>

                        {/* Composer */}
                        <div className="border-t border-[color:var(--color-border)]/60 bg-[color:var(--color-bg-2)]/60 p-3 backdrop-blur">
                          <div className="flex items-center gap-2 rounded-xl border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/60 px-3 py-2">
                            <span className="text-[11px] text-[color:var(--color-text-dim)]">Ask vibn anything…</span>
                            <div className="flex-1" />
                            <button className="rounded-md p-1 text-[color:var(--color-text-dim)] hover:bg-white/5">
                              <span className="block h-3.5 w-3.5 rounded-full border border-current" />
                            </button>
                            <button
                              className="flex h-6 w-6 items-center justify-center rounded-md text-white"
                              style={{ background: "linear-gradient(135deg, var(--color-brand-1), var(--color-brand-2))" }}
                              aria-label="Send"
                            >
                              <ArrowRight className="h-3.5 w-3.5" />
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Subtle inner edge highlight */}
                  <div className="pointer-events-none absolute inset-0 rounded-[12px] ring-1 ring-inset ring-white/[0.03]" />
                </div>

              </div>
            </motion.div>
          </div>
        </div>
      </div>
    </section>
  );
}
