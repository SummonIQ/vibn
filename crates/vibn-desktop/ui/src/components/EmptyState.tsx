import { motion } from "framer-motion";
import { Logo } from "./Logo";
import {
  IconCode,
  IconImage,
  IconSparkles,
  IconBrain,
  IconTerminal,
  IconPlug,
  IconSearch,
  IconBolt,
} from "./Icons";
import type { FC, SVGProps } from "react";

interface Props {
  activeModel: string;
  onAction: (cmd: string, args?: string) => void;
  onPromptHint: (text: string) => void;
}

interface Capability {
  Icon: FC<SVGProps<SVGSVGElement>>;
  title: string;
  body: string;
}

const CAPABILITIES: Capability[] = [
  { Icon: IconCode, title: "Code", body: "Read, edit, refactor, debug, run tests." },
  { Icon: IconImage, title: "Vision", body: "Describe images, OCR, sample video frames." },
  { Icon: IconSparkles, title: "Generate", body: "Local SDXL / Flux via managed ComfyUI." },
  { Icon: IconBrain, title: "Remember", body: "Per-project observations + memory." },
  { Icon: IconPlug, title: "MCP", body: "Connect Model Context Protocol servers." },
  { Icon: IconTerminal, title: "Shell", body: "Run commands with diff review + approval." },
  { Icon: IconSearch, title: "Search", body: "Grep code, find files, navigate repos." },
  { Icon: IconBolt, title: "Tools", body: "Tool-calling agent loop with auto-compact." },
];

const PROMPTS = [
  "Explain the agent loop in vibn-core",
  "What changed on this branch?",
  "Generate a cartoon character for my daughter",
  "Read the file at ./README.md",
  "Search for TODO across the repo",
  "Summarize the last 5 commits",
];

export function EmptyState({ activeModel, onAction, onPromptHint }: Props) {
  return (
    <div className="min-h-full grid place-items-center px-8 py-10">
      <motion.div
        initial={{ opacity: 0, y: 14 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.45, ease: [0.22, 1, 0.36, 1] }}
        className="w-full max-w-[720px] flex flex-col items-center gap-7"
      >
        {/* Hero logo with breathing glow */}
        <div className="relative">
          <motion.div
            className="absolute inset-0 rounded-3xl"
            style={{
              background:
                "radial-gradient(circle, rgba(167,139,250,0.35), transparent 65%)",
              filter: "blur(20px)",
            }}
            animate={{ opacity: [0.45, 0.85, 0.45], scale: [0.95, 1.1, 0.95] }}
            transition={{ duration: 4.5, repeat: Infinity, ease: "easeInOut" }}
          />
          <motion.div
            animate={{ scale: [1, 1.03, 1], rotate: [0, 0.8, -0.8, 0] }}
            transition={{ duration: 5, repeat: Infinity, ease: "easeInOut" }}
            className="relative"
          >
            <Logo size={56} />
          </motion.div>
        </div>

        <div className="text-center">
          <motion.h1
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.08, duration: 0.5, ease: [0.22, 1, 0.36, 1] }}
            className="text-2xl font-semibold tracking-tight"
          >
            <span className="bg-gradient-to-br from-violet-300 to-purple-400 bg-clip-text text-transparent">
              What can Vibn do for you?
            </span>
          </motion.h1>
          <motion.p
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ delay: 0.18, duration: 0.5 }}
            className="mt-2 text-[12.5px] text-white/45"
          >
            {activeModel ? (
              <>
                using <span className="text-white/70 font-medium">{activeModel}</span>
              </>
            ) : (
              "no model selected"
            )}
          </motion.p>
        </div>

        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.24, duration: 0.5 }}
          className="grid grid-cols-2 gap-2 w-full"
        >
          {CAPABILITIES.map((c, i) => (
            <CapCard key={c.title} cap={c} delay={0.3 + i * 0.03} />
          ))}
        </motion.div>

        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.55, duration: 0.5 }}
          className="w-full mt-3"
        >
          <div className="text-[10.5px] uppercase tracking-[0.1em] text-white/30 mb-2 text-center">try a prompt</div>
          <div className="flex flex-col gap-1.5">
            {PROMPTS.map((p, i) => (
              <motion.button
                key={p}
                onClick={() => onPromptHint(p)}
                whileHover={{
                  borderColor: "rgba(167,139,250,0.30)",
                  backgroundColor: "rgba(255,255,255,0.035)",
                }}
                whileTap={{ scale: 0.997 }}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.6 + i * 0.04 }}
                className="group text-left flex items-center gap-2.5 bg-zinc-900/40 border border-white/[0.06] rounded-xl px-3 py-2.5 transition-colors"
              >
                <span className="text-white/25 group-hover:text-violet-300/60 transition-colors flex-shrink-0">
                  <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2 8l12-6-6 12-2-5-4-1z" />
                  </svg>
                </span>
                <span className="text-[13px] text-white/55 group-hover:text-white/85 transition-colors truncate">
                  {p}
                </span>
              </motion.button>
            ))}
          </div>
        </motion.div>

        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.72 }}
          className="flex items-center gap-3 text-[11px] text-white/35"
        >
          <span>Type <kbd className="px-1.5 py-0.5 rounded bg-white/[0.06] border border-white/10 font-mono text-[10px]">/</kbd> for commands</span>
          <span>·</span>
          <button onClick={() => onAction("/help", "")} className="hover:text-white/70 transition-colors underline underline-offset-2">browse all commands</button>
        </motion.div>
      </motion.div>
    </div>
  );
}

// --- Capability card with subtle sparkle/glow on hover ---

interface SparklePos {
  top?: string;
  right?: string;
  bottom?: string;
  left?: string;
  delay: number;
}

interface SparkleParticle {
  origin: number;
  x: number;
  y: number;
  delay: number;
  color: "gold" | "silver";
  size: number;
}

const SPARKLES: SparklePos[] = [
  { top: "-3px", left: "12%", delay: 0 },
  { top: "-2px", right: "20%", delay: 0.4 },
  { bottom: "-2px", left: "30%", delay: 0.9 },
  { bottom: "-3px", right: "10%", delay: 1.3 },
  { top: "35%", left: "-3px", delay: 0.6 },
  { top: "60%", right: "-2px", delay: 1.1 },
];

const PARTICLES: SparkleParticle[] = [
  { origin: 0, x: -14, y: -18, delay: 0.05, color: "gold", size: 4 },
  { origin: 0, x: 18, y: -12, delay: 0.32, color: "silver", size: 3 },
  { origin: 1, x: 16, y: -20, delay: 0.18, color: "silver", size: 4 },
  { origin: 1, x: -12, y: -14, delay: 0.55, color: "gold", size: 3 },
  { origin: 2, x: -18, y: 15, delay: 0.22, color: "gold", size: 3 },
  { origin: 2, x: 12, y: 18, delay: 0.62, color: "silver", size: 4 },
  { origin: 3, x: 20, y: 16, delay: 0.35, color: "gold", size: 4 },
  { origin: 3, x: -15, y: 12, delay: 0.75, color: "silver", size: 3 },
  { origin: 4, x: -22, y: -8, delay: 0.12, color: "silver", size: 3 },
  { origin: 4, x: -18, y: 15, delay: 0.48, color: "gold", size: 4 },
  { origin: 5, x: 22, y: -10, delay: 0.28, color: "gold", size: 3 },
  { origin: 5, x: 18, y: 16, delay: 0.68, color: "silver", size: 4 },
];

const PARTICLE_COLOR = {
  gold: {
    background: "rgba(255, 211, 108, 0.96)",
    boxShadow: "0 0 7px rgba(255, 210, 92, 0.85), 0 0 2px rgba(255, 246, 199, 0.9)",
  },
  silver: {
    background: "rgba(232, 235, 240, 0.96)",
    boxShadow: "0 0 7px rgba(216, 222, 232, 0.8), 0 0 2px rgba(255, 255, 255, 0.85)",
  },
} as const;

function CapCard({ cap, delay }: { cap: Capability; delay: number }) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay, duration: 0.4 }}
      whileHover="hover"
      className="relative group rounded-xl bg-white/[0.025] border border-white/[0.07] p-3 flex items-start gap-3 cursor-default transition-all duration-300 ease-out hover:border-violet-400/25 hover:-translate-y-[1px] hover:bg-white/[0.035] hover:shadow-[0_0_28px_-10px_rgba(167,139,250,0.55),0_0_0_1px_rgba(167,139,250,0.18)_inset]"
    >
      {SPARKLES.map((s, i) => (
        <motion.span
          key={i}
          variants={{
            hover: {
              opacity: [0, 1, 0],
              scale: [0.4, 1.1, 0.4],
              transition: {
                duration: 1.6,
                repeat: Infinity,
                delay: s.delay,
                ease: "easeInOut",
              },
            },
          }}
          initial={{ opacity: 0, scale: 0.4 }}
          className="pointer-events-none absolute"
          style={{
            top: s.top,
            left: s.left,
            right: s.right,
            bottom: s.bottom,
          }}
        >
          <span
            className="block h-[3px] w-[3px] rounded-full"
            style={{
              background: "rgba(216, 200, 255, 0.95)",
            boxShadow: "0 0 6px rgba(167, 139, 250, 0.9), 0 0 2px rgba(216, 200, 255, 0.7)",
            }}
          />
        </motion.span>
      ))}

      {PARTICLES.map((p, i) => {
        const origin = SPARKLES[p.origin];
        return (
          <motion.span
            key={`particle-${i}`}
            variants={{
              hover: {
                opacity: [0, 1, 0],
                x: [0, p.x],
                y: [0, p.y],
                scale: [0.2, 1, 0.1],
                transition: {
                  duration: 1.15,
                  repeat: Infinity,
                  delay: p.delay,
                  ease: "easeOut",
                },
              },
            }}
            initial={{ opacity: 0, x: 0, y: 0, scale: 0.2 }}
            className="pointer-events-none absolute"
            style={{
              top: origin.top,
              left: origin.left,
              right: origin.right,
              bottom: origin.bottom,
            }}
            aria-hidden
          >
            <span
              className="block rounded-full"
              style={{
                width: `${p.size}px`,
                height: `${p.size}px`,
                ...PARTICLE_COLOR[p.color],
              }}
            />
          </motion.span>
        );
      })}

      <div className="relative h-8 w-8 rounded-md bg-gradient-to-br from-violet-500/25 to-purple-700/15 border border-violet-400/15 grid place-items-center text-violet-300 flex-shrink-0">
        <cap.Icon className="h-4 w-4" />
      </div>
      <div className="relative min-w-0">
        <div className="text-[12px] font-semibold leading-tight">{cap.title}</div>
        <div className="text-[11px] text-white/45 leading-snug mt-0.5">{cap.body}</div>
      </div>
    </motion.div>
  );
}
