import { useEffect, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { ProjectPicker } from "./ProjectPicker";
import type { ActiveProjectState } from "../types";

interface Props {
  title: string;
  subtitle?: string;
  canDelete: boolean;
  onDelete: () => void;
  onActiveProjectChange?: (state: ActiveProjectState) => void;
  explorerOpen?: boolean;
  canShowExplorer?: boolean;
  onToggleExplorer?: () => void;
}

export function ChatHeader({
  title,
  subtitle,
  canDelete,
  onDelete,
  onActiveProjectChange,
  explorerOpen,
  canShowExplorer,
  onToggleExplorer,
}: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!menuOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setMenuOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenuOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [menuOpen]);

  return (
    <header className="flex items-center justify-between gap-4 px-5 h-12 border-b border-white/5 bg-zinc-950/60 backdrop-blur">
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-semibold truncate">{title}</div>
        {subtitle && <div className="text-[10.5px] text-white/40 truncate">{subtitle}</div>}
      </div>
      <ProjectPicker onChange={onActiveProjectChange} />
      {canShowExplorer && onToggleExplorer && (
        <button
          type="button"
          onClick={onToggleExplorer}
          aria-label={explorerOpen ? "Hide file explorer" : "Show file explorer"}
          title={explorerOpen ? "Hide file explorer" : "Show file explorer"}
          className={
            "h-7 w-7 grid place-items-center rounded-md transition-colors border " +
            (explorerOpen
              ? "border-violet-500/40 bg-violet-500/[0.08] text-violet-200"
              : "border-white/10 bg-white/[0.03] text-white/55 hover:text-white hover:bg-white/[0.06]")
          }
        >
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
            <path d="M9 4l-5 8 5 8M15 4l5 8-5 8" />
          </svg>
        </button>
      )}
      <div ref={rootRef} className="relative">
        <button
          type="button"
          onClick={() => setMenuOpen((o) => !o)}
          aria-label="More actions"
          className="h-7 w-7 grid place-items-center rounded-md text-white/55 hover:text-white hover:bg-white/[0.06] transition-colors"
        >
          {/* lucide more-horizontal */}
          <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
            <circle cx="5" cy="12" r="1.6" />
            <circle cx="12" cy="12" r="1.6" />
            <circle cx="19" cy="12" r="1.6" />
          </svg>
        </button>
        <AnimatePresence>
          {menuOpen && (
            <motion.div
              initial={{ opacity: 0, y: -4, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -4, scale: 0.98 }}
              transition={{ duration: 0.14, ease: [0.22, 1, 0.36, 1] }}
              className="absolute right-0 mt-1 min-w-[180px] bg-zinc-900/95 backdrop-blur-md border border-white/10 rounded-lg shadow-2xl shadow-black/60 overflow-hidden z-50 py-1"
            >
              <button
                type="button"
                disabled={!canDelete}
                onClick={() => {
                  setMenuOpen(false);
                  if (canDelete) onDelete();
                }}
                className="w-full text-left px-3 py-1.5 text-[12.5px] text-red-300/85 hover:bg-red-500/[0.10] hover:text-red-200 disabled:opacity-40 disabled:cursor-not-allowed transition-colors flex items-center gap-2"
              >
                <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6M10 11v6M14 11v6" />
                </svg>
                Delete conversation
              </button>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </header>
  );
}
