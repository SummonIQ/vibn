import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import type { ActiveProjectState, ProjectInfo } from "../types";

function basename(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  return idx === -1 ? trimmed : trimmed.slice(idx + 1) || trimmed;
}

function shortPath(path: string): string {
  const home = (typeof window !== "undefined" && (window as { __HOME__?: string }).__HOME__) || "";
  let p = path;
  if (home && p.startsWith(home)) p = "~" + p.slice(home.length);
  if (p.length <= 38) return p;
  return "…" + p.slice(p.length - 37);
}

function FolderIcon({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    </svg>
  );
}

function CheckIcon({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M5 12l5 5L20 7" />
    </svg>
  );
}

interface Props {
  onChange?: (state: ActiveProjectState) => void;
}

export function ProjectPicker({ onChange }: Props) {
  const [state, setState] = useState<ActiveProjectState>({ active: null, recent: [] });
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const apply = useCallback(
    (next: ActiveProjectState) => {
      setState(next);
      onChange?.(next);
    },
    [onChange],
  );

  useEffect(() => {
    (async () => {
      try {
        apply(await api.getActiveProject());
      } catch {
        /* ignore */
      }
    })();
  }, [apply]);

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

  const pick = useCallback(async () => {
    try {
      const picked = await openDialog({ directory: true, multiple: false, title: "Open project" });
      if (typeof picked === "string" && picked) {
        apply(await api.setActiveProject(picked));
        setOpen(false);
      }
    } catch (err) {
      console.error("[vibn] pick project failed", err);
    }
  }, [apply]);

  const select = useCallback(
    async (project: ProjectInfo) => {
      try {
        apply(await api.setActiveProject(project.path));
        setOpen(false);
      } catch (err) {
        console.error("[vibn] set project failed", err);
      }
    },
    [apply],
  );

  const forget = useCallback(
    async (project: ProjectInfo, ev: React.MouseEvent) => {
      ev.stopPropagation();
      try {
        apply(await api.forgetRecentProject(project.path));
      } catch (err) {
        console.error("[vibn] forget project failed", err);
      }
    },
    [apply],
  );

  const clear = useCallback(async () => {
    try {
      apply(await api.clearActiveProject());
      setOpen(false);
    } catch (err) {
      console.error("[vibn] clear project failed", err);
    }
  }, [apply]);

  const label = state.active ? state.active.name : "No project";

  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        title={state.active ? state.active.path : "Open a project folder"}
        className={
          "flex items-center gap-1.5 h-7 px-2.5 rounded-md text-[11.5px] font-medium transition-colors border " +
          (state.active
            ? "border-violet-500/30 bg-violet-500/[0.06] text-violet-200 hover:bg-violet-500/[0.10]"
            : "border-white/10 bg-white/[0.03] text-white/55 hover:text-white hover:bg-white/[0.06]")
        }
      >
        <FolderIcon />
        <span className="max-w-[160px] truncate">{label}</span>
        <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="opacity-70">
          <path d="M6 9l6 6 6-6" />
        </svg>
      </button>
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: -4, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -4, scale: 0.98 }}
            transition={{ duration: 0.14, ease: [0.22, 1, 0.36, 1] }}
            className="absolute right-0 mt-1 min-w-[260px] max-w-[340px] bg-zinc-900/95 backdrop-blur-md border border-white/10 rounded-lg shadow-2xl shadow-black/60 overflow-hidden z-50 py-1"
          >
            {state.active && (
              <>
                <div className="px-3 pt-1.5 pb-1 text-[10px] uppercase tracking-wider text-white/35">Active</div>
                <button
                  type="button"
                  onClick={clear}
                  className="w-full text-left px-3 py-1.5 text-[12.5px] flex items-center gap-2 text-violet-200 hover:bg-white/[0.04] transition-colors"
                  title="Clear active project"
                >
                  <CheckIcon className="text-violet-300" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate">{state.active.name}</div>
                    <div className="truncate text-[10.5px] text-white/40">{shortPath(state.active.path)}</div>
                  </div>
                  <span className="text-[10px] text-white/30">Clear</span>
                </button>
                <div className="my-1 h-px bg-white/[0.06]" />
              </>
            )}

            {state.recent.length > 0 && (
              <>
                <div className="px-3 pt-1 pb-1 text-[10px] uppercase tracking-wider text-white/35">Recent</div>
                {state.recent
                  .filter((p) => !state.active || p.path !== state.active.path)
                  .slice(0, 8)
                  .map((p) => (
                    <div
                      key={p.path}
                      className="group flex items-center gap-2 px-3 py-1.5 hover:bg-white/[0.04] transition-colors"
                    >
                      <button
                        type="button"
                        onClick={() => select(p)}
                        className="flex-1 min-w-0 text-left flex items-center gap-2 text-[12.5px] text-white/85"
                      >
                        <FolderIcon className="text-white/40" />
                        <div className="min-w-0 flex-1">
                          <div className="truncate">{p.name}</div>
                          <div className="truncate text-[10.5px] text-white/35">{shortPath(p.path)}</div>
                        </div>
                      </button>
                      <button
                        type="button"
                        onClick={(ev) => forget(p, ev)}
                        title="Forget"
                        className="opacity-0 group-hover:opacity-100 text-white/40 hover:text-red-300 transition-opacity"
                      >
                        <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M6 6l12 12M18 6L6 18" />
                        </svg>
                      </button>
                    </div>
                  ))}
                <div className="my-1 h-px bg-white/[0.06]" />
              </>
            )}

            <button
              type="button"
              onClick={pick}
              className="w-full text-left px-3 py-1.5 text-[12.5px] flex items-center gap-2 text-white/85 hover:bg-white/[0.04] transition-colors"
            >
              <FolderIcon className="text-violet-300" />
              <span>Open folder…</span>
              <span className="ml-auto text-[10px] text-white/30">⌘O</span>
            </button>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
