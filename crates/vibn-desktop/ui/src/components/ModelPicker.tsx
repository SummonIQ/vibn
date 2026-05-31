import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { motion, AnimatePresence } from "framer-motion";
import { api } from "../api";
import type { ModelEntry, RegistryModel } from "../types";
import { ModelBrowser } from "./ModelBrowser";
import { cn } from "../lib/utils";

interface Props {
  installed: ModelEntry[];
  value: string;
  onChange: (m: string) => void;
  /** Render as a compact icon-only trigger (for the composer). */
  compact?: boolean;
}

function inferSummary(name: string): string {
  const lc = name.toLowerCase();
  if (lc.includes("embed") || lc.includes("nomic-embed")) return "Embedding model — vectorizes text for search";
  if (lc.includes("vision") || lc.includes("vl") || lc.includes("llava") || lc.includes("moondream") || lc.includes("minicpm-v")) return "Vision-capable — reads images";
  if (lc.includes("coder") || lc.includes("code")) return "Code-tuned model";
  if (lc.includes("fill")) return "Fill-in-the-middle code completion";
  if (lc.includes("deepseek")) return "DeepSeek model";
  if (lc.includes("flux")) return "Flux image-generation checkpoint";
  if (lc.includes("sdxl") || lc.includes("sd_xl") || lc.includes("sd-xl")) return "Stable Diffusion XL image gen";
  if (lc.includes("instruct") || lc.includes("chat")) return "Instruction-tuned chat model";
  if (lc.includes("llama")) return "Meta Llama base model";
  if (lc.includes("qwen")) return "Alibaba Qwen model";
  if (lc.includes("gemma")) return "Google Gemma model";
  if (lc.includes("mistral") || lc.includes("mixtral")) return "Mistral model";
  return "Ollama model";
}

export function ModelPicker({ installed, value, onChange, compact = false }: Props) {
  const [open, setOpen] = useState(false);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [registry, setRegistry] = useState<RegistryModel[]>([]);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [anchor, setAnchor] = useState<{
    top?: number;
    bottom?: number;
    right: number;
  } | null>(null);
  useLayoutEffect(() => {
    if (!open || !triggerRef.current) return;
    const r = triggerRef.current.getBoundingClientRect();
    const right = window.innerWidth - r.right;
    const openUpward = r.bottom + 420 > window.innerHeight;
    if (openUpward) {
      setAnchor({ bottom: window.innerHeight - r.top + 4, right });
    } else {
      setAnchor({ top: r.bottom + 4, right });
    }
  }, [open]);

  useEffect(() => {
    api.listModelRegistry().then(setRegistry).catch(() => setRegistry([]));
  }, []);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const installedNames = new Set(installed.map((m) => m.name));
  if (value) installedNames.add(value);

  const items = Array.from(installedNames).map((name) => {
    const reg = registry.find((r) => r.key === name);
    return {
      key: name,
      summary: reg?.summary ?? inferSummary(name),
      installed: true,
      reg,
    };
  });

  const currentReg = registry.find((r) => r.key === value);
  const currentSummary = currentReg?.summary ?? inferSummary(value);

  return (
    <>
      <div ref={rootRef} className="relative">
        {compact ? (
          <button
            ref={triggerRef}
            type="button"
            onClick={() => setOpen((o) => !o)}
            title={value ? `${value}${currentSummary ? ` — ${currentSummary}` : ""}` : "Pick model"}
            aria-label="Pick model"
            className="h-8 px-2.5 inline-flex items-center gap-1.5 rounded-lg text-[11.5px] bg-white/[0.04] border border-white/10 text-white/80 hover:bg-white/[0.07] hover:text-white transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-violet-400/50"
          >
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
              <rect x="6" y="6" width="12" height="12" rx="1.5" />
              <path d="M9 6V3M12 6V3M15 6V3M9 21v-3M12 21v-3M15 21v-3M6 9H3M6 12H3M6 15H3M21 9h-3M21 12h-3M21 15h-3" />
            </svg>
            <span className="max-w-[120px] truncate font-mono">{value || "model"}</span>
            <svg viewBox="0 0 12 12" className="h-2.5 w-2.5 opacity-50 flex-shrink-0" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path d="M3 4.5l3 3 3-3" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        ) : (
          <button
            ref={triggerRef}
            type="button"
            onClick={() => setOpen((o) => !o)}
            className="inline-flex items-center justify-between gap-2 min-w-[240px] max-w-[300px] h-auto py-1 px-2.5 rounded-md text-[12px] bg-white/[0.04] border border-white/10 text-white/90 hover:bg-white/[0.07] transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-violet-400/50"
          >
            <span className="flex flex-col items-start min-w-0 leading-tight">
              <span className="font-mono text-[12px] truncate w-full text-left">
                {value || <span className="text-white/40">No model</span>}
              </span>
              {value && currentSummary && (
                <span className="text-[10px] text-white/40 truncate w-full text-left">
                  {currentSummary}
                </span>
              )}
            </span>
            <svg viewBox="0 0 12 12" className="h-2.5 w-2.5 opacity-50 flex-shrink-0" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path d="M3 4.5l3 3 3-3" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        )}

        <AnimatePresence>
          {open && anchor && createPortal(
            <motion.div
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.14, ease: [0.22, 1, 0.36, 1] }}
              style={{
                position: "fixed",
                top: anchor.top,
                bottom: anchor.bottom,
                right: anchor.right,
                zIndex: 9999,
              }}
              className="min-w-[360px] max-h-[420px] overflow-hidden bg-zinc-950/95 backdrop-blur-xl border border-white/10 rounded-lg shadow-2xl shadow-black/80 flex flex-col"
            >
              <div className="overflow-y-auto vibn-scroll py-1 flex-1">
                <div className="px-2.5 pt-1.5 pb-1 text-[9.5px] uppercase tracking-[0.1em] text-white/30 font-semibold">
                  Installed
                </div>
                {items.length === 0 && (
                  <div className="px-2.5 py-2 text-[12px] text-white/40">
                    No Ollama models installed yet.
                  </div>
                )}
                {items.map((it) => (
                  <button
                    key={it.key}
                    type="button"
                    onClick={() => {
                      onChange(it.key);
                      setOpen(false);
                    }}
                    className={cn(
                      "w-full text-left px-2.5 py-1.5 flex flex-col gap-0.5 hover:bg-white/[0.06] transition-colors",
                      it.key === value ? "text-white" : "text-white/85",
                    )}
                  >
                    <div className="flex items-center gap-2">
                      {it.key === value ? (
                        <svg viewBox="0 0 12 12" className="h-2.5 w-2.5 text-violet-400 flex-shrink-0" fill="none" stroke="currentColor" strokeWidth="2.2">
                          <path d="M2 6l3 3 5-6" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                      ) : (
                        <span className="h-2.5 w-2.5 flex-shrink-0" />
                      )}
                      <span className="text-[12.5px] font-medium font-mono truncate">{it.key}</span>
                      {it.reg && (
                        <ModelBadges reg={it.reg} />
                      )}
                    </div>
                    {it.summary && (
                      <span className="text-[10.5px] text-white/45 truncate ml-[18px]">
                        {it.summary}
                      </span>
                    )}
                  </button>
                ))}
              </div>
              <div className="border-t border-white/5 p-1">
                <button
                  type="button"
                  onClick={() => {
                    setOpen(false);
                    setBrowserOpen(true);
                  }}
                  className="w-full text-left px-2.5 py-2 rounded-md hover:bg-violet-500/10 transition-colors flex items-center gap-2 text-[12.5px] text-violet-300"
                >
                  <svg viewBox="0 0 12 12" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
                    <path d="M6 2v8M2 6h8" />
                  </svg>
                  Browse all models…
                </button>
              </div>
            </motion.div>,
            document.body,
          )}
        </AnimatePresence>
      </div>

      <ModelBrowser
        open={browserOpen}
        onClose={() => setBrowserOpen(false)}
        registry={registry}
        installed={installedNames}
        onInstalled={(name) => {
          // refresh installed list — caller listens to install events; we also
          // optimistically set the active model when first installed.
          onChange(name);
        }}
      />
    </>
  );
}

function ModelBadges({ reg }: { reg: RegistryModel }) {
  return (
    <span className="flex items-center gap-1 flex-shrink-0">
      {reg.vision && (
        <span className="px-1.5 py-px rounded-full text-[9.5px] bg-violet-400/15 text-violet-200/85 border border-violet-400/20">
          vision
        </span>
      )}
      {reg.tool_support && (
        <span className="px-1.5 py-px rounded-full text-[9.5px] bg-emerald-400/15 text-emerald-200/85 border border-emerald-400/20">
          tools
        </span>
      )}
    </span>
  );
}
