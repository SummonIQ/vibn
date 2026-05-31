import { useEffect, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import type { ModelEntry, SlashCommandEntry } from "../types";
import { ModelPicker } from "./ModelPicker";
import { cn } from "../lib/utils";

interface Props {
  slashCommands: SlashCommandEntry[];
  busy: boolean;
  onSubmit: (text: string) => void;
  installedModels: ModelEntry[];
  activeModel: string;
  onChangeModel: (m: string) => void;
}

export function Composer({
  slashCommands,
  busy,
  onSubmit,
  installedModels,
  activeModel,
  onChangeModel,
}: Props) {
  const [text, setText] = useState("");
  const [paletteSel, setPaletteSel] = useState(0);
  const taRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const ta = taRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = `${Math.min(ta.scrollHeight, 220)}px`;
  }, [text]);

  const paletteOpen = text.startsWith("/");
  const palette = (() => {
    if (!paletteOpen) return [];
    const space = text.indexOf(" ");
    const q = (space === -1 ? text : text.slice(0, space)).slice(1).toLowerCase();
    return slashCommands
      .filter(
        (c) =>
          c.command.toLowerCase().includes(q) ||
          c.description.toLowerCase().includes(q),
      )
      .slice(0, 14);
  })();
  useEffect(() => {
    setPaletteSel(0);
  }, [text]);

  const submit = () => {
    if (busy) return;
    const t = text;
    if (!t.trim()) return;
    setText("");
    onSubmit(t);
  };

  const commitPalette = (idx: number) => {
    const c = palette[idx];
    if (!c) return;
    const space = text.indexOf(" ");
    const args = space === -1 ? "" : text.slice(space + 1);
    setText("");
    onSubmit(`${c.command}${args ? " " + args : ""}`);
  };

  return (
    <div className="relative px-5 pb-5 pt-2">
      <AnimatePresence>
        {paletteOpen && palette.length > 0 && (
          <motion.div
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 6 }}
            transition={{ duration: 0.14, ease: [0.22, 1, 0.36, 1] }}
            className="absolute left-5 right-5 bottom-full mb-2 bg-zinc-900/95 backdrop-blur-md border border-white/10 rounded-xl shadow-2xl shadow-black/60 overflow-hidden max-h-[280px] overflow-y-auto vibn-scroll z-30"
          >
            {palette.map((c, i) => (
              <button
                key={c.command}
                onMouseDown={(e) => {
                  e.preventDefault();
                  commitPalette(i);
                }}
                className={cn(
                  "w-full grid grid-cols-[180px_1fr] gap-3 px-3 py-1.5 text-[12.5px] text-left transition-colors",
                  i === paletteSel ? "bg-violet-500/15" : "hover:bg-white/[0.05]",
                )}
              >
                <span className="font-mono font-semibold text-violet-300 truncate">{c.command}</span>
                <span className="text-white/55 truncate">{c.description}</span>
              </button>
            ))}
          </motion.div>
        )}
      </AnimatePresence>

      <div className="relative flex items-end gap-2 bg-zinc-900/70 border border-white/10 rounded-2xl px-3 py-2.5 focus-within:border-violet-400/40 focus-within:bg-zinc-900 transition-all">
        <textarea
          ref={taRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (paletteOpen && palette.length > 0) {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setPaletteSel((s) => (s + 1) % palette.length);
                return;
              }
              if (e.key === "ArrowUp") {
                e.preventDefault();
                setPaletteSel((s) => (s - 1 + palette.length) % palette.length);
                return;
              }
              if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey)) {
                e.preventDefault();
                commitPalette(paletteSel);
                return;
              }
              if (e.key === "Escape") {
                e.preventDefault();
                setText("");
                return;
              }
            }
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
              e.preventDefault();
              submit();
            } else if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
          placeholder="Message Vibn…   /  for commands   ⌘↵ to send"
          rows={1}
          className="flex-1 resize-none bg-transparent text-[13.5px] leading-relaxed text-white placeholder:text-white/30 focus:outline-none min-h-[24px] max-h-[220px]"
        />

        <ModelPicker
          installed={installedModels}
          value={activeModel}
          onChange={onChangeModel}
          compact
        />

        <button
          type="button"
          onClick={submit}
          disabled={busy || !text.trim()}
          aria-label="Send"
          className={cn(
            "h-8 w-8 rounded-xl grid place-items-center flex-shrink-0 transition-all",
            "bg-gradient-to-br from-violet-500/65 to-purple-700/45 text-white",
            "border border-violet-400/40 border-b-black/40",
            "disabled:opacity-40 disabled:cursor-not-allowed",
            "hover:shadow-[0_4px_18px_rgba(139,92,246,0.4)] active:translate-y-[0.5px]",
          )}
        >
          {busy ? (
            <motion.span
              className="h-3 w-3 rounded-full border-2 border-current border-r-transparent"
              animate={{ rotate: 360 }}
              transition={{ duration: 0.9, repeat: Infinity, ease: "linear" }}
            />
          ) : (
            <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M2 8l12-6-6 12-2-5-4-1z" />
            </svg>
          )}
        </button>
      </div>
    </div>
  );
}
